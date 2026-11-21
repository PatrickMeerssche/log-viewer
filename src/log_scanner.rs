use crate::models::{ConversionStatus, LogEntry, TableMeta};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub fn safe_stem(stem: &str) -> String {
    stem.chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn scan_logs_directory(logs_dir: &Path, csv_base_dir: &Path) -> Vec<LogEntry> {
    let mut entries = Vec::new();

    if !logs_dir.exists() {
        return entries;
    }

    let read_dir = match fs::read_dir(logs_dir) {
        Ok(rd) => rd,
        Err(_) => return entries,
    };

    for item in read_dir.flatten() {
        let path = item.path();
        if !path.is_file() {
            continue;
        }

        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if filename.starts_with('.') {
            continue;
        }

        let extension = path
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let metadata = item.metadata().ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = metadata.and_then(|m| m.modified().ok());

        let safe = safe_stem(&stem);
        let output_dir = csv_base_dir.join(&safe);

        let tables = scan_tables_for_log(&path, &output_dir, csv_base_dir);
        let status = if extension.eq_ignore_ascii_case("csv") {
            ConversionStatus::Converted { table_count: 1 }
        } else if !tables.is_empty() {
            ConversionStatus::Converted {
                table_count: tables.len(),
            }
        } else {
            ConversionStatus::NotConverted
        };

        entries.push(LogEntry {
            filename,
            path,
            size_bytes,
            modified,
            extension,
            status,
            output_dir,
        });
    }

    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    entries
}

pub fn scan_tables_for_log(
    log_path: &Path,
    output_dir: &Path,
    csv_base_dir: &Path,
) -> Vec<TableMeta> {
    let mut tables = Vec::new();
    let ext = log_path
        .extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if ext == "csv" {
        if let Some(meta) = inspect_table_file(log_path, None) {
            tables.push(meta);
        }
        return tables;
    }

    let stem = log_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let safe = safe_stem(&stem);

    // 1. Check inside output_dir: csv/<safe_stem>/*.csv
    if output_dir.exists() && output_dir.is_dir() {
        if let Ok(rd) = fs::read_dir(output_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map(|e| e == "csv").unwrap_or(false) {
                    let fname = p.file_name().unwrap().to_string_lossy().to_string();
                    let table_name = parse_table_name(&fname, &safe);
                    if let Some(meta) = inspect_table_file(&p, Some(table_name)) {
                        tables.push(meta);
                    }
                }
            }
        }
    }

    // 2. Check in csv_base_dir: csv/<safe_stem>__*.csv
    if tables.is_empty() && csv_base_dir.exists() && csv_base_dir.is_dir() {
        let prefix = format!("{}__", safe);
        if let Ok(rd) = fs::read_dir(csv_base_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map(|e| e == "csv").unwrap_or(false) {
                    let fname = p.file_name().unwrap().to_string_lossy().to_string();
                    if fname.starts_with(&prefix) {
                        let table_name = parse_table_name(&fname, &safe);
                        if let Some(meta) = inspect_table_file(&p, Some(table_name)) {
                            tables.push(meta);
                        }
                    }
                }
            }
        }
    }

    // 3. Check in csv_out: csv_out/<safe_stem>__*.csv
    let csv_out_dir = PathBuf::from("csv_out");
    if tables.is_empty() && csv_out_dir.exists() && csv_out_dir.is_dir() {
        let prefix = format!("{}__", safe);
        if let Ok(rd) = fs::read_dir(&csv_out_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map(|e| e == "csv").unwrap_or(false) {
                    let fname = p.file_name().unwrap().to_string_lossy().to_string();
                    if fname.starts_with(&prefix) {
                        let table_name = parse_table_name(&fname, &safe);
                        if let Some(meta) = inspect_table_file(&p, Some(table_name)) {
                            tables.push(meta);
                        }
                    }
                }
            }
        }
    }

    tables.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    tables
}

fn parse_table_name(filename: &str, safe_stem: &str) -> String {
    let stem = filename.strip_suffix(".csv").unwrap_or(filename);
    let prefix = format!("{}__", safe_stem);
    if let Some(stripped) = stem.strip_prefix(&prefix) {
        stripped.to_string()
    } else if let Some(idx) = stem.rfind("__") {
        stem[idx + 2..].to_string()
    } else {
        stem.to_string()
    }
}

pub fn inspect_table_file(path: &Path, explicit_name: Option<String>) -> Option<TableMeta> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let file_size_bytes = metadata.len();

    let name = explicit_name.unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Table".to_string())
    });

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let header_line = match lines.next() {
        Some(Ok(l)) => l,
        _ => return None,
    };

    let col_count = header_line.split(',').count();

    // Fast count remaining lines
    let mut row_count = 0;
    for line in lines {
        if let Ok(l) = line {
            if !l.trim().is_empty() {
                row_count += 1;
            }
        }
    }

    Some(TableMeta {
        name,
        file_name: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        file_path: path.to_path_buf(),
        row_count,
        col_count,
        file_size_bytes,
    })
}

pub fn convert_log_sync(
    log_path: &Path,
    output_dir: &Path,
    _script_path: &Path,
) -> Result<(usize, String), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output dir: {}", e))?;

    let log = flightlog_core::Log::open(log_path, &flightlog_core::ReadOptions::default())
        .map_err(|e| format!("Failed to read {}: {}", log_path.display(), e))?;
    let base = safe_stem(
        &log_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let time_col = log.time_column();
    let utc_offset_us = log.utc_offset_us.unwrap_or(0);

    let mut table_count = 0;
    for table in log.tables() {
        let out_file = output_dir.join(format!("{}__{}.csv", base, table.name));
        let mut w = csv::Writer::from_path(&out_file)
            .map_err(|e| format!("Failed to write {}: {}", out_file.display(), e))?;
        let time = table.column(time_col).map(|c| &c.data);
        let mut header: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
        if time.is_some() {
            header.push("_timestamp");
        }
        w.write_record(&header).map_err(|e| e.to_string())?;
        for row in 0..table.rows {
            let mut rec: Vec<String> = table.columns.iter().map(|c| cell(&c.data, row)).collect();
            if let Some(t) = time.and_then(|t| t.get_i64(row)) {
                rec.push(((t + utc_offset_us) as f64 / 1e6).to_string());
            }
            w.write_record(&rec).map_err(|e| e.to_string())?;
        }
        w.flush().map_err(|e| e.to_string())?;
        table_count += 1;
    }

    let msg = format!(
        "Converted {}: {} messages -> {} CSV files",
        log_path.display(),
        log.integrity.messages,
        table_count
    );
    Ok((table_count, msg))
}

fn cell(data: &flightlog_core::ColumnData, row: usize) -> String {
    use flightlog_core::ColumnData::*;
    match data {
        Float32(v) => v[row].to_string(),
        Float64(v) => v[row].to_string(),
        Utf8(_) => data.get_str(row).unwrap_or_default().to_string(),
        Binary(v) => v.get(row).iter().map(|b| format!("{:02x}", b)).collect(),
        FixedList { size, values } => {
            let items: Vec<String> = (row * size..(row + 1) * size)
                .map(|i| cell(values, i))
                .collect();
            format!("[{}]", items.join(", "))
        }
        _ => data
            .get_i64(row)
            .map(|v| v.to_string())
            .unwrap_or_else(|| data.get_f64(row).map(|v| v.to_string()).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_stem() {
        assert_eq!(safe_stem("2026-06-10 14-31-26"), "2026-06-10_14-31-26");
        assert_eq!(safe_stem("log_494_UnknownDate"), "log_494_UnknownDate");
        assert_eq!(safe_stem("flight!#1"), "flight__1");
    }

    #[test]
    fn test_parse_table_name() {
        assert_eq!(
            parse_table_name("2026-06-10_14-31-26__ATTITUDE.csv", "2026-06-10_14-31-26"),
            "ATTITUDE"
        );
        assert_eq!(
            parse_table_name("log_494_UnknownDate__BAT.csv", "log_494_UnknownDate"),
            "BAT"
        );
    }
    fn fmt_record(ty: u8, len: u8, name: &str, format: &str, cols: &str) -> Vec<u8> {
        let mut b = vec![0xA3, 0x95, 0x80, ty, len];
        for (text, width) in [(name, 4), (format, 16), (cols, 64)] {
            let mut f = text.as_bytes().to_vec();
            f.resize(width, 0);
            b.extend(f);
        }
        b
    }

    #[test]
    fn test_convert_bin_without_python() {
        let dir = std::env::temp_dir().join(format!("log_viewer_convert_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut bin = fmt_record(0x80, 89, "FMT", "BBnNZ", "Type,Length,Name,Format,Columns");
        bin.extend(fmt_record(0x81, 15, "BAT", "Qf", "TimeUS,Curr"));
        for (t, curr) in [(1_000_000u64, 12.5f32), (2_000_000, 13.25)] {
            bin.extend([0xA3, 0x95, 0x81]);
            bin.extend(t.to_le_bytes());
            bin.extend(curr.to_le_bytes());
        }
        let log_path = dir.join("flight 1.bin");
        fs::write(&log_path, &bin).unwrap();

        let out = dir.join("csv");
        let (tables, _) = convert_log_sync(&log_path, &out, Path::new("missing.py")).unwrap();
        assert_eq!(tables, 2);
        let csv = fs::read_to_string(out.join("flight_1__BAT.csv")).unwrap();
        assert_eq!(
            csv,
            "TimeUS,Curr,_timestamp\n1000000,12.5,1\n2000000,13.25,2\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

use crate::models::{ConversionStatus, LogEntry, TableMeta};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

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
    script_path: &Path,
) -> Result<(usize, String), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output dir: {}", e))?;

    let py_cmd = if Command::new("python3").arg("--version").output().is_ok() {
        "python3"
    } else {
        "python"
    };

    let output = Command::new(py_cmd)
        .arg(script_path)
        .arg(log_path)
        .arg("-o")
        .arg(output_dir)
        .output()
        .map_err(|e| format!("Failed to execute converter script: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "Conversion error (exit code {:?}):\n{}\n{}",
            output.status.code(),
            stderr.trim(),
            stdout.trim()
        ));
    }

    let mut table_count = 0;
    if let Ok(rd) = fs::read_dir(output_dir) {
        for entry in rd.flatten() {
            if entry.path().extension().map(|e| e == "csv").unwrap_or(false) {
                table_count += 1;
            }
        }
    }

    Ok((table_count, stdout))
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
}

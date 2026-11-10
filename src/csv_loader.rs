use crate::models::{ColumnStat, LoadedTable, SeriesData};
use csv::ReaderBuilder;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::path::Path;

pub fn load_table_data(path: &Path, table_name: &str) -> Result<LoadedTable, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open file: {}", e))?;
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    let headers_record = rdr
        .headers()
        .map_err(|e| format!("Failed to read CSV headers: {}", e))?;

    let headers: Vec<String> = headers_record
        .iter()
        .map(|s| s.trim().to_string())
        .collect();

    if headers.is_empty() {
        return Err("CSV file contains no column headers".to_string());
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    let num_cols = headers.len();

    // Stats tracking
    let mut numeric_flags = vec![true; num_cols];
    let mut counts = vec![0usize; num_cols];
    let mut null_counts = vec![0usize; num_cols];
    let mut sum_values = vec![0.0f64; num_cols];
    let mut min_values = vec![f64::INFINITY; num_cols];
    let mut max_values = vec![f64::NEG_INFINITY; num_cols];
    let mut numeric_values_store: Vec<Vec<f64>> = vec![Vec::new(); num_cols];

    for result in rdr.records() {
        let record = match result {
            Ok(r) => r,
            Err(_) => continue,
        };

        let mut row = Vec::with_capacity(num_cols);
        for col_idx in 0..num_cols {
            let val = record.get(col_idx).unwrap_or("").trim();
            row.push(val.to_string());

            counts[col_idx] += 1;

            if val.is_empty()
                || val.eq_ignore_ascii_case("none")
                || val.eq_ignore_ascii_case("nan")
                || val.eq_ignore_ascii_case("null")
            {
                null_counts[col_idx] += 1;
            } else if numeric_flags[col_idx] {
                if let Ok(num) = val.parse::<f64>() {
                    if num.is_finite() {
                        sum_values[col_idx] += num;
                        if num < min_values[col_idx] {
                            min_values[col_idx] = num;
                        }
                        if num > max_values[col_idx] {
                            max_values[col_idx] = num;
                        }
                        // Sample up to 50k values for std dev calculation
                        if numeric_values_store[col_idx].len() < 50_000 {
                            numeric_values_store[col_idx].push(num);
                        }
                    }
                } else {
                    // Not a number
                    numeric_flags[col_idx] = false;
                }
            }
        }
        rows.push(row);
    }

    let mut numeric_columns = HashSet::new();
    let mut stats = Vec::new();

    for col_idx in 0..num_cols {
        let name = headers[col_idx].clone();
        let total = counts[col_idx];
        let nulls = null_counts[col_idx];
        let valid_numeric = total.saturating_sub(nulls);

        let is_num = numeric_flags[col_idx] && valid_numeric > 0;
        if is_num {
            numeric_columns.insert(name.clone());

            let mean = if valid_numeric > 0 {
                Some(sum_values[col_idx] / valid_numeric as f64)
            } else {
                None
            };

            let std_dev = if let Some(m) = mean {
                let store = &numeric_values_store[col_idx];
                if store.len() > 1 {
                    let variance = store.iter().map(|&x| (x - m).powi(2)).sum::<f64>()
                        / (store.len() - 1) as f64;
                    Some(variance.sqrt())
                } else {
                    Some(0.0)
                }
            } else {
                None
            };

            let min = if min_values[col_idx].is_finite() {
                Some(min_values[col_idx])
            } else {
                None
            };
            let max = if max_values[col_idx].is_finite() {
                Some(max_values[col_idx])
            } else {
                None
            };

            stats.push(ColumnStat {
                name,
                is_numeric: true,
                total_count: total,
                null_count: nulls,
                min,
                max,
                mean,
                std_dev,
            });
        } else {
            stats.push(ColumnStat {
                name,
                is_numeric: false,
                total_count: total,
                null_count: nulls,
                min: None,
                max: None,
                mean: None,
                std_dev: None,
            });
        }
    }

    Ok(LoadedTable {
        name: table_name.to_string(),
        path: path.to_path_buf(),
        headers,
        rows,
        numeric_columns,
        stats,
    })
}

pub fn extract_series_data(
    table: &LoadedTable,
    x_col_name: &str,
    y_col_names: &[String],
    group_col_name: Option<&str>,
    downsample_max: usize,
    relative_time: bool,
    normalize_y: bool,
) -> Vec<SeriesData> {
    if y_col_names.is_empty() {
        return Vec::new();
    }

    let x_idx = if x_col_name == "__ROW_INDEX__" {
        None
    } else {
        table.headers.iter().position(|h| h == x_col_name)
    };

    let group_idx = group_col_name.and_then(|g| table.headers.iter().position(|h| h == g));

    // Calculate x_0 (starting X value) and scaling if relative time
    let x_0 = if relative_time && x_idx.is_some() {
        let idx = x_idx.unwrap();
        table
            .rows
            .iter()
            .filter_map(|r| r.get(idx).and_then(|s| s.parse::<f64>().ok()))
            .filter(|v| v.is_finite())
            .fold(f64::INFINITY, f64::min)
    } else {
        0.0
    };

    let x_lower = x_col_name.to_lowercase();
    let x_scale = if relative_time {
        if x_lower.contains("timeus") || x_lower.ends_with("_us") || x_lower.contains("usec") {
            1e-6
        } else if (x_lower.contains("timems") || x_lower.ends_with("_ms") || x_lower.contains("msec"))
            && !x_lower.contains("_timestamp")
        {
            1e-3
        } else {
            1.0
        }
    } else {
        1.0
    };

    // Check if group column has a reasonable number of unique values (between 2 and 12)
    let allow_grouping = if let Some(g_idx) = group_idx {
        let mut unique_groups = HashSet::new();
        for row in &table.rows {
            if let Some(val) = row.get(g_idx) {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    unique_groups.insert(trimmed);
                    if unique_groups.len() > 12 {
                        break;
                    }
                }
            }
        }
        unique_groups.len() >= 2 && unique_groups.len() <= 12
    } else {
        false
    };

    let mut result_series = Vec::new();

    for y_name in y_col_names {
        let y_idx = match table.headers.iter().position(|h| h == y_name) {
            Some(i) => i,
            None => continue,
        };

        // Group points: group_label -> Vec<[x, y]>
        let mut grouped_points: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();

        for (row_idx, row) in table.rows.iter().enumerate() {
            let x_val = match x_idx {
                Some(idx) => match row.get(idx).and_then(|s| s.parse::<f64>().ok()) {
                    Some(v) if v.is_finite() => {
                        if relative_time && x_0.is_finite() {
                            (v - x_0) * x_scale
                        } else {
                            v
                        }
                    }
                    _ => continue,
                },
                None => row_idx as f64,
            };

            let y_val = match row.get(y_idx).and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if v.is_finite() => v,
                _ => continue,
            };

            let group_key = if allow_grouping {
                match group_idx {
                    Some(g_idx) => {
                        let g_val = row.get(g_idx).map(|s| s.as_str()).unwrap_or("").trim();
                        if g_val.is_empty() {
                            "default".to_string()
                        } else {
                            g_val.to_string()
                        }
                    }
                    None => "default".to_string(),
                }
            } else {
                "default".to_string()
            };

            grouped_points.entry(group_key).or_default().push([x_val, y_val]);
        }

        let is_multi_group = allow_grouping
            && (grouped_points.len() > 1
                || (grouped_points.len() == 1 && !grouped_points.contains_key("default")));

        for (group_val, mut points) in grouped_points {
            if points.is_empty() {
                continue;
            }

            // Sort by X coordinate
            points.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));

            // Record raw min and max Y before decimation/normalization
            let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
            let max_y = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);

            let mut decimated = decimate_points(&points, downsample_max);

            if normalize_y {
                let range = max_y - min_y;
                for p in &mut decimated {
                    if range > 1e-9 {
                        p[1] = ((p[1] - min_y) / range) * 100.0;
                    } else {
                        p[1] = 50.0;
                    }
                }
            }

            let base_label = if is_multi_group {
                let group_var = group_col_name.unwrap_or("group");
                format!("{} ({}={})", y_name, group_var, group_val)
            } else {
                y_name.clone()
            };

            let label = if normalize_y {
                format!("{} [%]", base_label)
            } else {
                base_label
            };

            result_series.push(SeriesData {
                label,
                raw_y_name: y_name.clone(),
                points: decimated,
                min_y,
                max_y,
            });
        }
    }

    result_series
}

pub fn decimate_points(points: &[[f64; 2]], max_points: usize) -> Vec<[f64; 2]> {
    if max_points == 0 || points.len() <= max_points || max_points < 2 {
        return points.to_vec();
    }

    let mut sampled = Vec::with_capacity(max_points);
    let step = (points.len() - 1) as f64 / (max_points - 1) as f64;

    for i in 0..max_points {
        let idx = (i as f64 * step).round() as usize;
        let idx = idx.min(points.len() - 1);
        sampled.push(points[idx]);
    }

    sampled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimate_points() {
        let pts: Vec<[f64; 2]> = (0..100).map(|i| [i as f64, (i * 2) as f64]).collect();
        let decimated = decimate_points(&pts, 10);
        assert_eq!(decimated.len(), 10);
        assert_eq!(decimated.first().unwrap()[0], 0.0);
        assert_eq!(decimated.last().unwrap()[0], 99.0);

        // Under limit
        let small: Vec<[f64; 2]> = vec![[0.0, 1.0], [1.0, 2.0]];
        let small_dec = decimate_points(&small, 10);
        assert_eq!(small_dec.len(), 2);
    }
}

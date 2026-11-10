use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversionStatus {
    NotConverted,
    Converting,
    Converted { table_count: usize },
    Error(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub filename: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
    pub extension: String,
    pub status: ConversionStatus,
    pub output_dir: PathBuf,
}

impl LogEntry {
    #[allow(dead_code)]
    pub fn is_csv(&self) -> bool {
        self.extension.eq_ignore_ascii_case("csv")
    }

    #[allow(dead_code)]
    pub fn formatted_size(&self) -> String {
        format_size(self.size_bytes)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub name: String,
    pub file_name: String,
    pub file_path: PathBuf,
    pub row_count: usize,
    pub col_count: usize,
    pub file_size_bytes: u64,
}

impl TableMeta {
    #[allow(dead_code)]
    pub fn formatted_size(&self) -> String {
        format_size(self.file_size_bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStat {
    pub name: String,
    pub is_numeric: bool,
    pub total_count: usize,
    pub null_count: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub std_dev: Option<f64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LoadedTable {
    pub name: String,
    pub path: PathBuf,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub numeric_columns: HashSet<String>,
    pub stats: Vec<ColumnStat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Scatter,
    Step,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SeriesData {
    pub label: String,
    pub raw_y_name: String,
    pub points: Vec<[f64; 2]>,
    pub min_y: f64,
    pub max_y: f64,
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

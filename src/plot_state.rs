use crate::csv_loader::extract_series_data;
use crate::models::{ChartType, LoadedTable, SeriesData};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct PlotState {
    pub selected_x: String,
    pub selected_y: BTreeSet<String>,
    pub group_by_col: Option<String>,
    pub chart_type: ChartType,
    pub line_width: f32,
    pub downsample_limit: usize,
    pub show_grid: bool,
    pub show_legend: bool,
    pub normalize_y: bool,
    pub relative_time: bool,
    pub request_reset_bounds: bool,
    pub column_search: String,
    pub cached_series: Vec<SeriesData>,
    pub is_dirty: bool,
    pub var_panel_width: f32,
}

impl Default for PlotState {
    fn default() -> Self {
        Self {
            selected_x: String::new(),
            selected_y: BTreeSet::new(),
            group_by_col: None,
            chart_type: ChartType::Line,
            line_width: 1.5,
            downsample_limit: 10_000,
            show_grid: true,
            show_legend: true,
            normalize_y: false,
            relative_time: true,
            request_reset_bounds: true,
            column_search: String::new(),
            cached_series: Vec::new(),
            is_dirty: true,
            var_panel_width: 250.0,
        }
    }
}

impl PlotState {
    pub fn init_for_table(&mut self, table: &LoadedTable) {
        self.request_reset_bounds = true;
        self.group_by_col = None;

        // Pick best default X axis: _timestamp -> TimeUS -> TimeMS -> __ROW_INDEX__
        let x_candidate = table
            .headers
            .iter()
            .find(|h| h.eq_ignore_ascii_case("_timestamp"))
            .or_else(|| {
                table
                    .headers
                    .iter()
                    .find(|h| h.eq_ignore_ascii_case("timeus"))
            })
            .or_else(|| {
                table
                    .headers
                    .iter()
                    .find(|h| h.eq_ignore_ascii_case("timems"))
            });

        self.selected_x = match x_candidate {
            Some(name) => name.clone(),
            None => {
                if let Some(first_num) = table.stats.iter().find(|s| s.is_numeric).map(|s| &s.name) {
                    first_num.clone()
                } else {
                    "__ROW_INDEX__".to_string()
                }
            }
        };

        // Pick default Y columns:
        self.selected_y.clear();

        // Look for common interesting telemetry fields: Curr, Volt, Roll, Pitch, Yaw, Alt, Speed
        let interesting = [
            "curr",
            "current_battery",
            "volt",
            "roll",
            "pitch",
            "yaw",
            "alt",
            "speed",
            "accx",
            "gyrx",
        ];

        for interest in &interesting {
            if let Some(col) = table.stats.iter().find(|s| {
                s.is_numeric
                    && s.name.eq_ignore_ascii_case(interest)
                    && s.name != self.selected_x
            }) {
                self.selected_y.insert(col.name.clone());
                break;
            }
        }

        // If none found, pick first numeric column that is not X
        if self.selected_y.is_empty() {
            if let Some(first_num) = table
                .stats
                .iter()
                .find(|s| s.is_numeric && s.name != self.selected_x)
            {
                self.selected_y.insert(first_num.name.clone());
            }
        }

        self.is_dirty = true;
    }

    pub fn update_series_if_dirty(&mut self, table: &LoadedTable) {
        if self.is_dirty {
            let y_cols: Vec<String> = self.selected_y.iter().cloned().collect();
            self.cached_series = extract_series_data(
                table,
                &self.selected_x,
                &y_cols,
                self.group_by_col.as_deref(),
                self.downsample_limit,
                self.relative_time,
                self.normalize_y,
            );
            self.is_dirty = false;
        }
    }
}

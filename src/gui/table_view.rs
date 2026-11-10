use crate::models::LoadedTable;
use egui::{Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use std::cmp::Ordering;

pub struct TableViewState {
    pub search_query: String,
    pub sort_column: Option<usize>,
    pub sort_ascending: bool,
    pub current_page: usize,
    pub page_size: usize,
    pub filtered_indices: Vec<usize>,
    pub last_row_count: usize,
    pub is_dirty: bool,
}

impl Default for TableViewState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            sort_column: None,
            sort_ascending: true,
            current_page: 0,
            page_size: 50,
            filtered_indices: Vec::new(),
            last_row_count: 0,
            is_dirty: true,
        }
    }
}

impl TableViewState {
    pub fn update_indices(&mut self, table: &LoadedTable) {
        if !self.is_dirty && self.last_row_count == table.rows.len() {
            return;
        }

        let query = self.search_query.trim().to_lowercase();
        let mut indices: Vec<usize> = if query.is_empty() {
            (0..table.rows.len()).collect()
        } else {
            table
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.iter()
                        .any(|cell| cell.to_lowercase().contains(&query))
                })
                .map(|(i, _)| i)
                .collect()
        };

        if let Some(col_idx) = self.sort_column {
            let asc = self.sort_ascending;
            let is_numeric = table.headers.get(col_idx).map(|h| table.numeric_columns.contains(h)).unwrap_or(false);

            indices.sort_by(|&a, &b| {
                let val_a = table.rows[a].get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let val_b = table.rows[b].get(col_idx).map(|s| s.as_str()).unwrap_or("");

                let ord = if is_numeric {
                    let num_a = val_a.parse::<f64>().unwrap_or(f64::NAN);
                    let num_b = val_b.parse::<f64>().unwrap_or(f64::NAN);
                    num_a.partial_cmp(&num_b).unwrap_or(Ordering::Equal)
                } else {
                    val_a.cmp(val_b)
                };

                if asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }

        self.filtered_indices = indices;
        self.last_row_count = table.rows.len();
        self.is_dirty = false;

        let max_page = (self.filtered_indices.len().saturating_sub(1)) / self.page_size;
        if self.current_page > max_page {
            self.current_page = max_page;
        }
    }
}

pub fn render_table_view(
    ui: &mut Ui,
    table: &LoadedTable,
    state: &mut TableViewState,
) {
    state.update_indices(table);

    ui.vertical(|ui| {
        // Controls Row
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍 Search:").size(13.0));
            let edit_resp = ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Filter cells across all columns...")
                    .desired_width(260.0),
            );
            if edit_resp.changed() {
                state.is_dirty = true;
            }

            if !state.search_query.is_empty() && ui.button("✖ Clear").clicked() {
                state.search_query.clear();
                state.is_dirty = true;
            }

            ui.separator();

            // Page size
            ui.label("Rows per page:");
            egui::ComboBox::from_id_salt("page_size_combo")
                .selected_text(format!("{}", state.page_size))
                .width(60.0)
                .show_ui(ui, |ui| {
                    for &size in &[25, 50, 100, 250, 500, 1000] {
                        if ui
                            .selectable_value(&mut state.page_size, size, format!("{}", size))
                            .clicked()
                        {
                            state.current_page = 0;
                            state.is_dirty = true;
                        }
                    }
                });

            ui.separator();

            // Export to CSV button
            if ui.button("💾 Export Filtered CSV").on_hover_text("Save current filtered/sorted rows to CSV").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(&format!("{}_export.csv", table.name))
                    .add_filter("CSV", &["csv"])
                    .save_file()
                {
                    export_filtered_csv(table, &state.filtered_indices, &path);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let total = table.rows.len();
                let filtered = state.filtered_indices.len();
                if filtered == total {
                    ui.label(RichText::new(format!("Total: {} rows", total)).color(Color32::from_rgb(148, 163, 184)));
                } else {
                    ui.label(
                        RichText::new(format!("Showing {} of {} rows", filtered, total))
                            .color(Color32::from_rgb(251, 191, 36)),
                    );
                }
            });
        });

        ui.add_space(4.0);

        // Pagination Bar
        let total_items = state.filtered_indices.len();
        let total_pages = if total_items == 0 {
            1
        } else {
            (total_items + state.page_size - 1) / state.page_size
        };

        ui.horizontal(|ui| {
            if ui.button("⏮").on_hover_text("First page").clicked() && state.current_page > 0 {
                state.current_page = 0;
            }
            if ui.button("◀").on_hover_text("Previous page").clicked() && state.current_page > 0 {
                state.current_page -= 1;
            }

            ui.label(
                RichText::new(format!(
                    "Page {} of {}",
                    state.current_page + 1,
                    total_pages
                ))
                .strong(),
            );

            if ui.button("▶").on_hover_text("Next page").clicked()
                && state.current_page + 1 < total_pages
            {
                state.current_page += 1;
            }
            if ui.button("⏭").on_hover_text("Last page").clicked()
                && state.current_page + 1 < total_pages
            {
                state.current_page = total_pages - 1;
            }

            let start_idx = if total_items == 0 {
                0
            } else {
                state.current_page * state.page_size + 1
            };
            let end_idx = ((state.current_page + 1) * state.page_size).min(total_items);
            ui.label(
                RichText::new(format!("(Rows {}-{})", start_idx, end_idx))
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        });

        ui.add_space(4.0);

        // The Table
        let page_start = state.current_page * state.page_size;
        let page_end = (page_start + state.page_size).min(total_items);
        let current_page_indices = if page_start < total_items {
            &state.filtered_indices[page_start..page_end]
        } else {
            &[]
        };

        let row_height = 20.0;
        let mut table_builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(50.0)); // # index column

        for _ in 0..table.headers.len() {
            table_builder = table_builder.column(Column::initial(110.0).at_least(60.0));
        }

        table_builder
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.strong("#");
                });

                for (idx, h) in table.headers.iter().enumerate() {
                    let sort_indicator = if state.sort_column == Some(idx) {
                        if state.sort_ascending { " ▲" } else { " ▼" }
                    } else {
                        ""
                    };

                    header.col(|ui| {
                        let text = format!("{}{}", h, sort_indicator);
                        let is_sorted = state.sort_column == Some(idx);
                        let rich = if is_sorted {
                            RichText::new(text).strong().color(Color32::from_rgb(147, 197, 253))
                        } else {
                            RichText::new(text).strong()
                        };

                        if ui.button(rich).on_hover_text("Click to sort").clicked() {
                            if state.sort_column == Some(idx) {
                                if state.sort_ascending {
                                    state.sort_ascending = false;
                                } else {
                                    state.sort_column = None;
                                    state.sort_ascending = true;
                                }
                            } else {
                                state.sort_column = Some(idx);
                                state.sort_ascending = true;
                            }
                            state.is_dirty = true;
                        }
                    });
                }
            })
            .body(|mut body| {
                for (disp_idx, &row_idx) in current_page_indices.iter().enumerate() {
                    let row = &table.rows[row_idx];
                    let global_idx = page_start + disp_idx + 1;

                    body.row(row_height, |mut row_ui| {
                        row_ui.col(|ui| {
                            ui.label(RichText::new(format!("{}", global_idx)).size(11.0).color(Color32::GRAY));
                        });

                        for (col_idx, cell) in row.iter().enumerate() {
                            row_ui.col(|ui| {
                                let is_numeric = table.headers.get(col_idx).map(|h| table.numeric_columns.contains(h)).unwrap_or(false);
                                if is_numeric {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(cell).monospace().size(11.5));
                                    });
                                } else {
                                    ui.label(RichText::new(cell).size(11.5));
                                }
                            });
                        }
                    });
                }
            });
    });
}

fn export_filtered_csv(table: &LoadedTable, indices: &[usize], path: &std::path::Path) {
    if let Ok(mut writer) = csv::Writer::from_path(path) {
        let _ = writer.write_record(&table.headers);
        for &idx in indices {
            if let Some(row) = table.rows.get(idx) {
                let _ = writer.write_record(row);
            }
        }
        let _ = writer.flush();
    }
}

use crate::models::LoadedTable;
use egui::{Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder};

pub enum StatsAction {
    None,
    PlotColumn(String),
}

pub fn render_stats_view(ui: &mut Ui, table: &LoadedTable) -> StatsAction {
    let mut action = StatsAction::None;

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("📊 Column Statistics").size(15.0).strong());
            ui.label(
                RichText::new(format!("({} columns)", table.stats.len()))
                    .size(12.0)
                    .color(Color32::from_rgb(148, 163, 184)),
            );
        });

        ui.add_space(4.0);

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(140.0).at_least(100.0)) // Column Name
            .column(Column::initial(80.0).at_least(60.0))   // Type
            .column(Column::initial(90.0).at_least(60.0))   // Total Count
            .column(Column::initial(80.0).at_least(50.0))   // Nulls
            .column(Column::initial(100.0).at_least(70.0))  // Min
            .column(Column::initial(100.0).at_least(70.0))  // Max
            .column(Column::initial(100.0).at_least(70.0))  // Mean
            .column(Column::initial(100.0).at_least(70.0))  // Std Dev
            .column(Column::initial(100.0).at_least(80.0))  // Action
            .header(24.0, |mut header| {
                header.col(|ui| { ui.strong("Column"); });
                header.col(|ui| { ui.strong("Type"); });
                header.col(|ui| { ui.strong("Count"); });
                header.col(|ui| { ui.strong("Nulls"); });
                header.col(|ui| { ui.strong("Min"); });
                header.col(|ui| { ui.strong("Max"); });
                header.col(|ui| { ui.strong("Mean"); });
                header.col(|ui| { ui.strong("Std Dev"); });
                header.col(|ui| { ui.strong("Action"); });
            })
            .body(|mut body| {
                for stat in &table.stats {
                    body.row(22.0, |mut row| {
                        row.col(|ui| {
                            ui.label(RichText::new(&stat.name).strong().size(12.0));
                        });

                        row.col(|ui| {
                            if stat.is_numeric {
                                ui.label(RichText::new("Number").color(Color32::from_rgb(147, 197, 253)).size(11.0));
                            } else {
                                ui.label(RichText::new("Text").color(Color32::GRAY).size(11.0));
                            }
                        });

                        row.col(|ui| {
                            ui.label(RichText::new(format!("{}", stat.total_count)).size(11.5));
                        });

                        row.col(|ui| {
                            if stat.null_count > 0 {
                                ui.label(RichText::new(format!("{}", stat.null_count)).color(Color32::from_rgb(251, 191, 36)).size(11.5));
                            } else {
                                ui.label(RichText::new("0").color(Color32::GRAY).size(11.5));
                            }
                        });

                        row.col(|ui| {
                            let text = stat.min.map(|v| format!("{:.4}", v)).unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new(text).monospace().size(11.5));
                        });

                        row.col(|ui| {
                            let text = stat.max.map(|v| format!("{:.4}", v)).unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new(text).monospace().size(11.5));
                        });

                        row.col(|ui| {
                            let text = stat.mean.map(|v| format!("{:.4}", v)).unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new(text).monospace().size(11.5));
                        });

                        row.col(|ui| {
                            let text = stat.std_dev.map(|v| format!("{:.4}", v)).unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new(text).monospace().size(11.5));
                        });

                        row.col(|ui| {
                            if stat.is_numeric {
                                if ui.button(RichText::new("📈 Plot").size(11.0)).clicked() {
                                    action = StatsAction::PlotColumn(stat.name.clone());
                                }
                            }
                        });
                    });
                }
            });
    });

    action
}

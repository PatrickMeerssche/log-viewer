use crate::models::{ConversionStatus, LogEntry, TableMeta};
use egui::{Color32, RichText, ScrollArea, Ui};

pub enum DetailsAction {
    None,
    ReconvertLog(String),
}

pub fn render_details_view(
    ui: &mut Ui,
    log: Option<&LogEntry>,
    tables: &[TableMeta],
    console_log: &str,
) -> DetailsAction {
    let mut action = DetailsAction::None;

    ui.vertical(|ui| {
        ui.heading(RichText::new("ℹ️ Log File & Conversion Details").size(15.0).strong());
        ui.add_space(8.0);

        if let Some(log) = log {
            egui::Grid::new("details_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Filename:").strong());
                    ui.label(&log.filename);
                    ui.end_row();

                    ui.label(RichText::new("Full Path:").strong());
                    ui.label(log.path.to_string_lossy());
                    ui.end_row();

                    ui.label(RichText::new("File Size:").strong());
                    ui.label(format!("{} ({} bytes)", log.formatted_size(), log.size_bytes));
                    ui.end_row();

                    ui.label(RichText::new("Format / Extension:").strong());
                    ui.label(log.extension.to_uppercase());
                    ui.end_row();

                    ui.label(RichText::new("Output Folder:").strong());
                    ui.label(log.output_dir.to_string_lossy());
                    ui.end_row();

                    ui.label(RichText::new("Status:").strong());
                    match &log.status {
                        ConversionStatus::Converted { table_count } => {
                            ui.label(
                                RichText::new(format!("Converted ({} tables generated)", table_count))
                                    .color(Color32::from_rgb(110, 231, 183)),
                            );
                        }
                        ConversionStatus::Converting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Converting in background...");
                            });
                        }
                        ConversionStatus::NotConverted => {
                            ui.label(RichText::new("Not yet converted").color(Color32::from_rgb(251, 191, 36)));
                        }
                        ConversionStatus::Error(err) => {
                            ui.label(RichText::new(format!("Error: {}", err)).color(Color32::from_rgb(248, 113, 113)));
                        }
                    }
                    ui.end_row();

                    ui.label(RichText::new("Total Tables:").strong());
                    ui.label(format!("{}", tables.len()));
                    ui.end_row();
                });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                if ui.button("🔄 Reconvert / Re-extract Tables").clicked() {
                    action = DetailsAction::ReconvertLog(log.filename.clone());
                }

                if ui.button("📂 Open Output Folder").clicked() {
                    let _ = webbrowser::open(&log.output_dir.to_string_lossy());
                }
            });
        } else {
            ui.label(RichText::new("No log selected.").italics().color(Color32::GRAY));
        }

        ui.add_space(16.0);
        ui.heading(RichText::new("📜 Conversion Log / Console Output").size(14.0).strong());
        ui.add_space(4.0);

        ScrollArea::vertical()
            .id_salt("console_log_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut console_log.to_string())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(ui.available_width())
                        .desired_rows(8)
                        .interactive(false),
                );
            });
    });

    action
}

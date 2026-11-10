use crate::models::{ConversionStatus, LogEntry, TableMeta};
use egui::{Color32, RichText, ScrollArea, Ui};

pub struct SidebarAction {
    pub selected_log: Option<String>,
    pub selected_table: Option<TableMeta>,
    pub convert_log: Option<String>,
    pub convert_all: bool,
    pub refresh_logs: bool,
    pub add_log_files: bool,
}

pub fn render_sidebar(
    ui: &mut Ui,
    logs: &[LogEntry],
    active_log_name: Option<&str>,
    tables: &[TableMeta],
    active_table_path: Option<&std::path::Path>,
    log_filter: &mut String,
    table_filter: &mut String,
) -> SidebarAction {
    let mut action = SidebarAction {
        selected_log: None,
        selected_table: None,
        convert_log: None,
        convert_all: false,
        refresh_logs: false,
        add_log_files: false,
    };

    ui.vertical(|ui| {
        // --- LOGS SECTION ---
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Logs").size(15.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(RichText::new("+ Add").size(11.0))
                    .on_hover_text("Add external log file (.bin, .tlog, .csv) into ./logs")
                    .clicked()
                {
                    action.add_log_files = true;
                }
                if ui
                    .button(RichText::new("Refresh").size(11.0))
                    .on_hover_text("Refresh ./logs")
                    .clicked()
                {
                    action.refresh_logs = true;
                }
                if ui
                    .button(RichText::new("Convert All").size(11.0))
                    .on_hover_text("Convert all unconverted logs")
                    .clicked()
                {
                    action.convert_all = true;
                }
            });
        });

        ui.add(
            egui::TextEdit::singleline(log_filter)
                .hint_text("Filter logs...")
                .desired_width(ui.available_width()),
        );

        ui.add_space(4.0);

        let filter_l = log_filter.to_lowercase();
        let filtered_logs: Vec<&LogEntry> = logs
            .iter()
            .filter(|l| filter_l.is_empty() || l.filename.to_lowercase().contains(&filter_l))
            .collect();

        // Top half for logs
        let half_height = (ui.available_height() - 100.0) * 0.45;
        ScrollArea::vertical()
            .id_salt("sidebar_logs_scroll")
            .max_height(half_height.max(120.0))
            .show(ui, |ui| {
                if filtered_logs.is_empty() {
                    ui.label(RichText::new("No logs found in ./logs").italics().color(Color32::GRAY));
                }

                for log in filtered_logs {
                    let is_active = active_log_name == Some(&log.filename);

                    let tag_name = log.extension.to_uppercase();
                    let tag_color = match log.extension.to_lowercase().as_str() {
                        "bin" => Color32::from_rgb(147, 197, 253),
                        "tlog" => Color32::from_rgb(253, 186, 116),
                        "csv" => Color32::from_rgb(110, 231, 183),
                        _ => Color32::from_rgb(148, 163, 184),
                    };

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&tag_name)
                                .size(9.0)
                                .monospace()
                                .color(tag_color),
                        );
                        let resp = ui.selectable_label(
                            is_active,
                            RichText::new(&log.filename)
                                .strong()
                                .size(12.5),
                        );
                        if resp.clicked() {
                            action.selected_log = Some(log.filename.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            match &log.status {
                                ConversionStatus::Converted { table_count } => {
                                    ui.label(
                                        RichText::new(format!("{} tbls", table_count))
                                            .size(10.0)
                                            .color(Color32::from_rgb(110, 231, 183)),
                                    );
                                }
                                ConversionStatus::Converting => {
                                    ui.spinner();
                                }
                                ConversionStatus::NotConverted => {
                                    if ui
                                        .button(RichText::new("Extract").size(10.0))
                                        .on_hover_text("Convert this log to CSV tables")
                                        .clicked()
                                    {
                                        action.convert_log = Some(log.filename.clone());
                                    }
                                }
                                ConversionStatus::Error(_) => {
                                    ui.label(
                                        RichText::new("Err")
                                            .size(10.0)
                                            .color(Color32::from_rgb(248, 113, 113)),
                                    );
                                }
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("      {} • {}", log.extension.to_uppercase(), log.formatted_size()))
                                .size(10.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    });

                    ui.separator();
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // --- TABLES SECTION ---
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Tables").size(15.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("({} tables)", tables.len()))
                        .size(11.0)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            });
        });

        ui.add(
            egui::TextEdit::singleline(table_filter)
                .hint_text("Filter tables (e.g. BAT, ATT)...")
                .desired_width(ui.available_width()),
        );

        ui.add_space(4.0);

        let filter_t = table_filter.to_lowercase();
        let filtered_tables: Vec<&TableMeta> = tables
            .iter()
            .filter(|t| filter_t.is_empty() || t.name.to_lowercase().contains(&filter_t))
            .collect();

        ScrollArea::vertical()
            .id_salt("sidebar_tables_scroll")
            .show(ui, |ui| {
                if tables.is_empty() {
                    ui.label(
                        RichText::new("No tables extracted yet.\nClick 'Extract' on a log above.")
                            .italics()
                            .color(Color32::GRAY),
                    );
                } else if filtered_tables.is_empty() {
                    ui.label(RichText::new("No matching tables.").italics().color(Color32::GRAY));
                }

                for table in filtered_tables {
                    let is_active = active_table_path == Some(table.file_path.as_path());

                    ui.horizontal(|ui| {
                        let resp = ui.selectable_label(
                            is_active,
                            RichText::new(&table.name)
                                .strong()
                                .size(12.0),
                        );
                        if resp.clicked() {
                            action.selected_table = Some(table.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{} rows", table.row_count))
                                    .size(10.0)
                                    .color(Color32::from_rgb(147, 197, 253)),
                            );
                        });
                    });
                }
            });
    });

    action
}

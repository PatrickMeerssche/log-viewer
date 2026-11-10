use crate::csv_loader::load_table_data;
use crate::gui::details_view::{render_details_view, DetailsAction};
use crate::gui::graph_view::render_graph_view;
use crate::gui::messages_view::{render_messages_view, MessagesViewState};
use crate::gui::sidebar::render_sidebar;
use crate::gui::stats_view::{render_stats_view, StatsAction};
use crate::gui::table_view::{render_table_view, TableViewState};
use crate::log_scanner::{convert_log_sync, scan_logs_directory, scan_tables_for_log};
use crate::messages_loader::{load_log_messages, LogMessageItem, MessageSeverity};
use crate::models::{ConversionStatus, LoadedTable, LogEntry, TableMeta};
use crate::plot_state::PlotState;
use crate::web_server::{start_web_server, WebServerHandle};
use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::{CentralPanel, Color32, Panel, RichText};
use std::path::PathBuf;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Graph,
    Table,
    Stats,
    Messages,
    Details,
}

pub struct TelemetryVisualizerApp {
    pub logs_dir: PathBuf,
    pub csv_dir: PathBuf,
    pub script_path: PathBuf,

    pub logs: Vec<LogEntry>,
    pub active_log_idx: Option<usize>,

    pub tables: Vec<TableMeta>,
    pub active_table_path: Option<PathBuf>,
    pub loaded_table: Option<LoadedTable>,

    pub messages: Vec<LogMessageItem>,
    pub messages_view_state: MessagesViewState,

    pub active_tab: ActiveTab,
    pub plot_state: PlotState,
    pub table_view_state: TableViewState,

    pub log_filter: String,
    pub table_filter: String,

    pub status_message: String,
    pub console_log: String,
    pub is_loading_table: bool,

    // Channels for background tasks
    conversion_tx: Sender<(String, Result<(usize, String), String>)>,
    conversion_rx: Receiver<(String, Result<(usize, String), String>)>,

    table_load_tx: Sender<Result<LoadedTable, String>>,
    table_load_rx: Receiver<Result<LoadedTable, String>>,

    pub web_server_handle: Option<WebServerHandle>,
}

impl TelemetryVisualizerApp {
    pub fn new(
        logs_dir: PathBuf,
        csv_dir: PathBuf,
        script_path: PathBuf,
    ) -> Self {
        let (conversion_tx, conversion_rx) = unbounded();
        let (table_load_tx, table_load_rx) = unbounded();

        let mut app = Self {
            logs_dir,
            csv_dir,
            script_path,
            logs: Vec::new(),
            active_log_idx: None,
            tables: Vec::new(),
            active_table_path: None,
            loaded_table: None,
            messages: Vec::new(),
            messages_view_state: MessagesViewState::default(),
            active_tab: ActiveTab::Graph,
            plot_state: PlotState::default(),
            table_view_state: TableViewState::default(),
            log_filter: String::new(),
            table_filter: String::new(),
            status_message: "Ready".to_string(),
            console_log: "Telemetry Log Visualizer initialized.\n".to_string(),
            is_loading_table: false,
            conversion_tx,
            conversion_rx,
            table_load_tx,
            table_load_rx,
            web_server_handle: None,
        };

        app.refresh_logs();
        if !app.logs.is_empty() {
            let best_idx = app
                .logs
                .iter()
                .position(|l| matches!(l.status, ConversionStatus::Converted { table_count } if table_count > 0))
                .unwrap_or(0);
            app.select_log_by_index(best_idx);
        }

        app
    }

    pub fn with_initial(mut self, log_name: Option<String>, tab: Option<ActiveTab>) -> Self {
        if let Some(ref name) = log_name {
            self.select_log_by_name(name);
        }
        if let Some(t) = tab {
            self.active_tab = t;
        }
        self
    }

    pub fn refresh_logs(&mut self) {
        self.logs = scan_logs_directory(&self.logs_dir, &self.csv_dir);
        self.status_message = format!("Found {} logs in {}", self.logs.len(), self.logs_dir.display());

        // Refresh tables and messages for active log if any
        if let Some(idx) = self.active_log_idx {
            if idx < self.logs.len() {
                let log = &self.logs[idx];
                self.tables = scan_tables_for_log(&log.path, &log.output_dir, &self.csv_dir);
                let stem = log.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let safe = crate::log_scanner::safe_stem(&stem);
                self.messages = load_log_messages(&log.output_dir, &self.csv_dir, &safe);
            }
        }
    }

    pub fn select_log_by_index(&mut self, idx: usize) {
        if idx >= self.logs.len() {
            return;
        }

        self.active_log_idx = Some(idx);
        let log = &self.logs[idx];
        self.tables = scan_tables_for_log(&log.path, &log.output_dir, &self.csv_dir);
        let stem = log.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let safe = crate::log_scanner::safe_stem(&stem);
        self.messages = load_log_messages(&log.output_dir, &self.csv_dir, &safe);

        let err_count = self.messages.iter().filter(|m| m.severity == MessageSeverity::Error).count();
        if err_count > 0 {
            self.status_message = format!(
                "Selected log: {} ({} tables, {} errors/messages)",
                log.filename,
                self.tables.len(),
                self.messages.len()
            );
        } else {
            self.status_message = format!("Selected log: {} ({} tables available)", log.filename, self.tables.len());
        }

        // If tables exist, auto-select interesting table (e.g. BAT, ATT) or first table
        if !self.tables.is_empty() {
            let interesting = ["BAT", "BATTERY_STATUS", "ATT", "ATTITUDE", "GPS", "BARO", "IMU"];
            let best_table = self
                .tables
                .iter()
                .find(|t| interesting.iter().any(|&intr| t.name.eq_ignore_ascii_case(intr)))
                .unwrap_or(&self.tables[0])
                .clone();
            self.load_table(&best_table);
        } else {
            self.active_table_path = None;
            self.loaded_table = None;
        }
    }

    pub fn select_log_by_name(&mut self, filename: &str) {
        if let Some(idx) = self.logs.iter().position(|l| l.filename == filename) {
            self.select_log_by_index(idx);
        }
    }

    pub fn load_table(&mut self, table_meta: &TableMeta) {
        self.active_table_path = Some(table_meta.file_path.clone());
        self.is_loading_table = true;
        self.status_message = format!("Loading table {}...", table_meta.name);

        let path = table_meta.file_path.clone();
        let name = table_meta.name.clone();
        let tx = self.table_load_tx.clone();

        thread::spawn(move || {
            let res = load_table_data(&path, &name);
            let _ = tx.send(res);
        });
    }

    pub fn start_conversion(&mut self, log_name: &str) {
        let log_entry = match self.logs.iter_mut().find(|l| l.filename == log_name) {
            Some(l) => l,
            None => return,
        };

        log_entry.status = ConversionStatus::Converting;
        self.status_message = format!("Converting {} in background...", log_name);

        let log_path = log_entry.path.clone();
        let output_dir = log_entry.output_dir.clone();
        let script = self.script_path.clone();
        let name = log_name.to_string();
        let tx = self.conversion_tx.clone();

        thread::spawn(move || {
            let res = convert_log_sync(&log_path, &output_dir, &script);
            let _ = tx.send((name, res));
        });
    }

    pub fn convert_all_unconverted(&mut self) {
        let names_to_convert: Vec<String> = self
            .logs
            .iter()
            .filter(|l| matches!(l.status, ConversionStatus::NotConverted))
            .map(|l| l.filename.clone())
            .collect();

        for name in names_to_convert {
            self.start_conversion(&name);
        }
    }

    fn check_background_messages(&mut self) {
        // Check conversion completions
        while let Ok((log_name, res)) = self.conversion_rx.try_recv() {
            match res {
                Ok((count, stdout)) => {
                    self.console_log.push_str(&format!(
                        "[{}] Successfully converted {}: {} tables created.\n{}\n",
                        chrono::Local::now().format("%H:%M:%S"),
                        log_name,
                        count,
                        stdout.trim()
                    ));

                    if let Some(entry) = self.logs.iter_mut().find(|l| l.filename == log_name) {
                        entry.status = ConversionStatus::Converted { table_count: count };
                    }

                    if let Some(idx) = self.active_log_idx {
                        if self.logs.get(idx).map(|l| l.filename == log_name).unwrap_or(false) {
                            let log = &self.logs[idx];
                            self.tables = scan_tables_for_log(&log.path, &log.output_dir, &self.csv_dir);
                            let stem = log.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            let safe = crate::log_scanner::safe_stem(&stem);
                            self.messages = load_log_messages(&log.output_dir, &self.csv_dir, &safe);
                            if !self.tables.is_empty() && self.loaded_table.is_none() {
                                let first = self.tables[0].clone();
                                self.load_table(&first);
                            }
                        }
                    }

                    self.status_message = format!("Converted {}: {} tables ready", log_name, count);
                }
                Err(err) => {
                    self.console_log.push_str(&format!(
                        "[{}] Error converting {}:\n{}\n",
                        chrono::Local::now().format("%H:%M:%S"),
                        log_name,
                        err
                    ));

                    if let Some(entry) = self.logs.iter_mut().find(|l| l.filename == log_name) {
                        entry.status = ConversionStatus::Error(err.clone());
                    }

                    self.status_message = format!("Failed to convert {}", log_name);
                }
            }
        }

        // Check table load completions
        while let Ok(res) = self.table_load_rx.try_recv() {
            self.is_loading_table = false;
            match res {
                Ok(loaded) => {
                    self.status_message = format!(
                        "Loaded table {}: {} rows, {} columns",
                        loaded.name,
                        loaded.rows.len(),
                        loaded.headers.len()
                    );
                    self.plot_state.init_for_table(&loaded);
                    self.table_view_state = TableViewState::default();
                    self.loaded_table = Some(loaded);
                }
                Err(err) => {
                    self.status_message = format!("Error loading table: {}", err);
                    self.console_log.push_str(&format!("Table load error: {}\n", err));
                }
            }
        }
    }

    pub fn launch_or_open_web_dashboard(&mut self) {
        if self.web_server_handle.is_none() {
            match start_web_server(
                self.logs_dir.clone(),
                self.csv_dir.clone(),
                self.script_path.clone(),
                8080,
            ) {
                Ok(handle) => {
                    let port = handle.port;
                    self.web_server_handle = Some(handle);
                    let url = format!("http://127.0.0.1:{}", port);
                    let _ = webbrowser::open(&url);
                    self.status_message = format!("Web dashboard launched at {}", url);
                }
                Err(e) => {
                    self.status_message = format!("Failed to start web server: {}", e);
                }
            }
        } else if let Some(ref handle) = self.web_server_handle {
            let url = format!("http://127.0.0.1:{}", handle.port);
            let _ = webbrowser::open(&url);
        }
    }
}

impl eframe::App for TelemetryVisualizerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.check_background_messages();

        if self.is_loading_table || self.logs.iter().any(|l| matches!(l.status, ConversionStatus::Converting)) {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        }

        // 1. TOP HEADER PANEL
        Panel::top("top_header_panel").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Telemetry & Flight Log Visualizer").size(16.0).strong());

                if let Some(idx) = self.active_log_idx {
                    if let Some(log) = self.logs.get(idx) {
                        ui.label(RichText::new("•").color(Color32::GRAY));
                        ui.label(
                            RichText::new(format!("Log: {}", log.filename))
                                .strong()
                                .color(Color32::from_rgb(147, 197, 253)),
                        );
                    }
                }

                if let Some(ref table) = self.loaded_table {
                    ui.label(RichText::new("•").color(Color32::GRAY));
                    ui.label(
                        RichText::new(format!("Table: {}", table.name))
                            .strong()
                            .color(Color32::from_rgb(110, 231, 183)),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("Web Dashboard").size(12.0))
                        .on_hover_text("Open interactive web browser dashboard")
                        .clicked()
                    {
                        self.launch_or_open_web_dashboard();
                    }

                    if self.is_loading_table {
                        ui.spinner();
                        ui.label(RichText::new("Loading...").size(11.0).color(Color32::GRAY));
                    } else {
                        ui.label(RichText::new(&self.status_message).size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    }
                });
            });
            ui.add_space(4.0);
        });

        // 2. LEFT SIDEBAR PANEL
        Panel::left("left_sidebar_panel")
            .resizable(true)
            .default_size(320.0)
            .size_range(240.0..=500.0)
            .show(ui, |ui| {
                let active_log_name = self.active_log_idx.and_then(|idx| self.logs.get(idx)).map(|l| l.filename.as_str());
                let active_table_p = self.active_table_path.as_deref();

                let action = render_sidebar(
                    ui,
                    &self.logs,
                    active_log_name,
                    &self.tables,
                    active_table_p,
                    &mut self.log_filter,
                    &mut self.table_filter,
                );

                if let Some(log_name) = action.selected_log {
                    self.select_log_by_name(&log_name);
                }

                if let Some(table_meta) = action.selected_table {
                    self.load_table(&table_meta);
                }

                if let Some(convert_name) = action.convert_log {
                    self.start_conversion(&convert_name);
                }

                if action.convert_all {
                    self.convert_all_unconverted();
                }

                if action.refresh_logs {
                    self.refresh_logs();
                }

                if action.add_log_files {
                    if let Some(files) = rfd::FileDialog::new()
                        .add_filter("Flight & Telemetry Logs", &["bin", "tlog", "csv", "log"])
                        .pick_files()
                    {
                        for file in files {
                            if let Some(fname) = file.file_name() {
                                let dest = self.logs_dir.join(fname);
                                let _ = std::fs::copy(&file, &dest);
                            }
                        }
                        self.refresh_logs();
                    }
                }
            });

        // 3. CENTRAL PANEL
        CentralPanel::default().show(ui, |ui| {
            let active_log = self.active_log_idx.and_then(|idx| self.logs.get(idx));

            if active_log.is_some() || self.loaded_table.is_some() {
                let err_count = self.messages.iter().filter(|m| m.severity == MessageSeverity::Error).count();
                let total_msg = self.messages.len();

                // Tab Navigation
                ui.horizontal(|ui| {
                    let mut tab_button = |tab: ActiveTab, label: &str, is_alert: bool| {
                        let is_active = self.active_tab == tab;
                        let mut text = RichText::new(label);
                        if is_active {
                            text = text.strong().color(Color32::WHITE);
                        } else if is_alert {
                            text = text.strong().color(Color32::from_rgb(252, 165, 165));
                        } else {
                            text = text.color(Color32::from_rgb(148, 163, 184));
                        }
                        let btn = if is_active {
                            egui::Button::new(text).fill(Color32::from_rgb(59, 130, 246))
                        } else if is_alert {
                            egui::Button::new(text).fill(Color32::from_rgba_premultiplied(127, 29, 29, 70))
                        } else {
                            egui::Button::new(text)
                        };

                        if ui.add(btn).clicked() {
                            self.active_tab = tab;
                        }
                    };

                    tab_button(ActiveTab::Graph, "Graph", false);
                    tab_button(ActiveTab::Table, "Table View", false);
                    tab_button(ActiveTab::Stats, "Column Stats", false);

                    let msg_label = if err_count > 0 {
                        format!("Messages & Errors ({})", err_count)
                    } else if total_msg > 0 {
                        format!("Messages ({})", total_msg)
                    } else {
                        "Messages & Errors".to_string()
                    };
                    tab_button(ActiveTab::Messages, &msg_label, err_count > 0);

                    tab_button(ActiveTab::Details, "Log Details", false);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(ref table) = self.loaded_table {
                            ui.label(
                                RichText::new(format!("{} rows • {} cols", table.rows.len(), table.headers.len()))
                                    .size(11.5)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            );
                        } else if active_log.is_some() {
                            ui.label(
                                RichText::new(format!("{} tables available", self.tables.len()))
                                    .size(11.5)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            );
                        }
                    });
                });

                ui.separator();

                // Tab Content
                match self.active_tab {
                    ActiveTab::Graph => {
                        if let Some(ref table) = self.loaded_table {
                            render_graph_view(ui, table, &mut self.plot_state);
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.heading(RichText::new("No Table Selected for Graph").size(16.0).color(Color32::GRAY));
                                    ui.add_space(8.0);
                                    ui.label("Select a telemetry table from the sidebar on the left (e.g. BAT, ATT, GPS).");
                                });
                            });
                        }
                    }
                    ActiveTab::Table => {
                        if let Some(ref table) = self.loaded_table {
                            render_table_view(ui, table, &mut self.table_view_state);
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.heading(RichText::new("No Table Selected").size(16.0).color(Color32::GRAY));
                                    ui.add_space(8.0);
                                    ui.label("Select a table from the sidebar to inspect its rows and columns.");
                                });
                            });
                        }
                    }
                    ActiveTab::Stats => {
                        if let Some(ref table) = self.loaded_table {
                            let action = render_stats_view(ui, table);
                            if let StatsAction::PlotColumn(col) = action {
                                self.plot_state.selected_y.clear();
                                self.plot_state.selected_y.insert(col);
                                self.plot_state.is_dirty = true;
                                self.active_tab = ActiveTab::Graph;
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.heading(RichText::new("No Table Selected").size(16.0).color(Color32::GRAY));
                                    ui.add_space(8.0);
                                    ui.label("Select a table from the sidebar to compute column statistics.");
                                });
                            });
                        }
                    }
                    ActiveTab::Messages => {
                        let log_name = active_log.map(|l| l.filename.as_str());
                        render_messages_view(ui, &self.messages, &mut self.messages_view_state, log_name);
                    }
                    ActiveTab::Details => {
                        let action = render_details_view(ui, active_log, &self.tables, &self.console_log);
                        if let DetailsAction::ReconvertLog(name) = action {
                            self.start_conversion(&name);
                        }
                    }
                }
            } else {
                // Empty State / Instructions
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(RichText::new("Telemetry & Flight Log Visualizer").size(22.0).strong());
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Select a log from the sidebar on the left to begin.")
                                .size(14.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                        ui.add_space(16.0);
                        ui.label(
                            "• ArduPilot / PX4 .bin files\n• MAVLink telemetry .tlog files\n• CSV tables & logs",
                        );
                        ui.add_space(20.0);
                        if ui.button(RichText::new("Refresh Logs Directory").size(14.0)).clicked() {
                            self.refresh_logs();
                        }
                    });
                });
            }
        });
    }
}

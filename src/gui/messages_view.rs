use crate::messages_loader::{format_rel_time, LogMessageItem, MessageSeverity};
use egui::{Color32, RichText, ScrollArea, Stroke, Ui, Vec2};

#[derive(Debug, Clone)]
pub struct MessagesViewState {
    pub search_query: String,
    pub severity_filter: Option<MessageSeverity>,
    pub source_filter: Option<String>,
}

impl Default for MessagesViewState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            severity_filter: None,
            source_filter: None,
        }
    }
}

pub enum MessagesAction {
    None,
}

pub fn render_messages_view(
    ui: &mut Ui,
    messages: &[LogMessageItem],
    state: &mut MessagesViewState,
    active_log_name: Option<&str>,
) -> MessagesAction {
    if messages.is_empty() {
        render_empty_state(ui, active_log_name);
        return MessagesAction::None;
    }

    // Count severities
    let error_count = messages.iter().filter(|m| m.severity == MessageSeverity::Error).count();
    let warn_count = messages.iter().filter(|m| m.severity == MessageSeverity::Warning).count();
    let event_count = messages.iter().filter(|m| m.severity == MessageSeverity::Event).count();
    let info_count = messages.iter().filter(|m| m.severity == MessageSeverity::Info).count();

    // 1. TOP SUMMARY CARDS
    ui.horizontal(|ui| {
        // Total pill
        render_stat_pill(
            ui,
            &format!("All ({})", messages.len()),
            state.severity_filter.is_none(),
            Color32::from_rgb(59, 130, 246),
            || {
                state.severity_filter = None;
            },
        );

        // Errors pill
        let err_color = Color32::from_rgb(239, 68, 68);
        render_stat_pill(
            ui,
            &format!("Errors ({})", error_count),
            state.severity_filter == Some(MessageSeverity::Error),
            err_color,
            || {
                state.severity_filter = if state.severity_filter == Some(MessageSeverity::Error) {
                    None
                } else {
                    Some(MessageSeverity::Error)
                };
            },
        );

        // Warnings pill
        let warn_color = Color32::from_rgb(245, 158, 11);
        render_stat_pill(
            ui,
            &format!("Warnings ({})", warn_count),
            state.severity_filter == Some(MessageSeverity::Warning),
            warn_color,
            || {
                state.severity_filter = if state.severity_filter == Some(MessageSeverity::Warning) {
                    None
                } else {
                    Some(MessageSeverity::Warning)
                };
            },
        );

        // Events pill
        let event_color = Color32::from_rgb(14, 165, 233);
        render_stat_pill(
            ui,
            &format!("Events ({})", event_count),
            state.severity_filter == Some(MessageSeverity::Event),
            event_color,
            || {
                state.severity_filter = if state.severity_filter == Some(MessageSeverity::Event) {
                    None
                } else {
                    Some(MessageSeverity::Event)
                };
            },
        );

        // Info pill
        let info_color = Color32::from_rgb(156, 163, 175);
        render_stat_pill(
            ui,
            &format!("ℹ️ Info ({})", info_count),
            state.severity_filter == Some(MessageSeverity::Info),
            info_color,
            || {
                state.severity_filter = if state.severity_filter == Some(MessageSeverity::Info) {
                    None
                } else {
                    Some(MessageSeverity::Info)
                };
            },
        );
    });

    ui.add_space(8.0);

    // 2. SEARCH & CONTROLS BAR
    let mut copy_all_text = None;

    ui.horizontal(|ui| {
        ui.label(RichText::new("Filter:").size(13.0));
        let _search_edit = ui.add(
            egui::TextEdit::singleline(&mut state.search_query)
                .hint_text("Filter by error text, PreArm, subsystem, code...")
                .desired_width(280.0),
        );

        if !state.search_query.is_empty() && ui.button("Clear").on_hover_text("Clear search").clicked() {
            state.search_query.clear();
        }

        ui.add_space(8.0);

        // Source dropdown
        ui.label(RichText::new("Source:").color(Color32::from_rgb(148, 163, 184)));
        let current_source = state.source_filter.clone().unwrap_or_else(|| "All".to_string());
        egui::ComboBox::from_id_salt("messages_source_filter")
            .selected_text(&current_source)
            .width(100.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(state.source_filter.is_none(), "All").clicked() {
                    state.source_filter = None;
                }
                for src in ["MSG", "STATUSTEXT", "ERR", "EV"] {
                    let is_sel = state.source_filter.as_deref() == Some(src);
                    if ui.selectable_label(is_sel, src).clicked() {
                        state.source_filter = Some(src.to_string());
                    }
                }
            });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(RichText::new("Copy All").size(12.0))
                .on_hover_text("Copy all currently visible messages to clipboard")
                .clicked()
            {
                copy_all_text = Some(String::new());
            }
        });
    });

    ui.separator();

    // Filter messages
    let query_lower = state.search_query.trim().to_lowercase();
    let filtered_messages: Vec<&LogMessageItem> = messages
        .iter()
        .filter(|msg| {
            if let Some(sev) = state.severity_filter {
                if msg.severity != sev {
                    return false;
                }
            }
            if let Some(ref src) = state.source_filter {
                if !msg.source.eq_ignore_ascii_case(src) {
                    return false;
                }
            }
            if !query_lower.is_empty() {
                let matches_msg = msg.message.to_lowercase().contains(&query_lower);
                let matches_src = msg.source.to_lowercase().contains(&query_lower);
                let matches_details = msg
                    .details
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false);
                if !matches_msg && !matches_src && !matches_details {
                    return false;
                }
            }
            true
        })
        .collect();

    // If user clicked copy all
    if let Some(mut text) = copy_all_text {
        for msg in &filtered_messages {
            text.push_str(&format!(
                "[{}] [{}] [{}] {}\n",
                format_rel_time(msg.rel_time_s),
                msg.severity.label(),
                msg.source,
                msg.message
            ));
        }
        ui.ctx().copy_text(text);
    }

    // Status counter
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "Showing {} of {} messages",
                filtered_messages.len(),
                messages.len()
            ))
            .size(11.5)
            .color(Color32::from_rgb(148, 163, 184)),
        );

        if state.severity_filter.is_some() || state.source_filter.is_some() || !state.search_query.is_empty() {
            if ui
                .link(RichText::new("Reset all filters").size(11.5).color(Color32::from_rgb(96, 165, 250)))
                .clicked()
            {
                state.severity_filter = None;
                state.source_filter = None;
                state.search_query.clear();
            }
        }
    });

    ui.add_space(4.0);

    // 3. MESSAGES TIMELINE LIST
    if filtered_messages.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(32.0);
                ui.heading(RichText::new("No matching messages").size(16.0).color(Color32::GRAY));
                ui.label("Try adjusting your search terms or severity filters.");
                if ui.button("Clear Search & Filters").clicked() {
                    state.severity_filter = None;
                    state.source_filter = None;
                    state.search_query.clear();
                }
            });
        });
        return MessagesAction::None;
    }

    let mut message_to_copy = None;

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (idx, msg) in filtered_messages.iter().enumerate() {
                let bg_color = match msg.severity {
                    MessageSeverity::Error => Color32::from_rgb(36, 20, 22),
                    MessageSeverity::Warning => Color32::from_rgb(34, 26, 16),
                    MessageSeverity::Event => Color32::from_rgb(18, 25, 38),
                    MessageSeverity::Info => {
                        if idx % 2 == 0 {
                            Color32::from_rgb(24, 29, 39)
                        } else {
                            Color32::from_rgb(18, 21, 29)
                        }
                    }
                };

                let (badge_bg, badge_fg, badge_label) = match msg.severity {
                    MessageSeverity::Error => (
                        Color32::from_rgb(127, 29, 29),
                        Color32::from_rgb(254, 202, 202),
                        "ERROR",
                    ),
                    MessageSeverity::Warning => (
                        Color32::from_rgb(120, 53, 15),
                        Color32::from_rgb(253, 230, 138),
                        "WARN",
                    ),
                    MessageSeverity::Event => (
                        Color32::from_rgb(30, 58, 138),
                        Color32::from_rgb(191, 219, 254),
                        "EVENT",
                    ),
                    MessageSeverity::Info => (
                        Color32::from_rgb(55, 65, 81),
                        Color32::from_rgb(209, 213, 219),
                        "INFO",
                    ),
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Relative time
                            let time_text = format_rel_time(msg.rel_time_s);
                            ui.label(
                                RichText::new(time_text)
                                    .monospace()
                                    .size(11.5)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            )
                            .on_hover_text(format!("Raw log timestamp: {:.3}s", msg.raw_time));

                            ui.add_space(4.0);

                            // Severity badge
                            let badge_frame = egui::Frame::new()
                                .fill(badge_bg)
                                .corner_radius(3.0)
                                .inner_margin(egui::Margin::symmetric(5, 2));
                            badge_frame.show(ui, |ui| {
                                ui.label(RichText::new(badge_label).size(10.5).strong().color(badge_fg));
                            });

                            ui.add_space(2.0);

                            // Source tag
                            let source_frame = egui::Frame::new()
                                .fill(Color32::from_rgb(30, 41, 59))
                                .corner_radius(3.0)
                                .inner_margin(egui::Margin::symmetric(4, 1));
                            source_frame.show(ui, |ui| {
                                ui.label(
                                    RichText::new(&msg.source)
                                        .monospace()
                                        .size(10.0)
                                        .color(Color32::from_rgb(148, 163, 184)),
                                );
                            });

                            ui.add_space(6.0);

                            // Message text
                            let msg_color = match msg.severity {
                                MessageSeverity::Error => Color32::from_rgb(252, 165, 165),
                                MessageSeverity::Warning => Color32::from_rgb(253, 224, 71),
                                MessageSeverity::Event => Color32::from_rgb(186, 230, 253),
                                MessageSeverity::Info => Color32::from_rgb(226, 232, 240),
                            };

                            ui.label(RichText::new(&msg.message).strong().color(msg_color));

                            // Optional details
                            if let Some(ref details) = msg.details {
                                ui.label(
                                    RichText::new(format!("({})", details))
                                        .size(11.0)
                                        .color(Color32::from_rgb(100, 116, 139)),
                                );
                            }

                            // Copy button on hover / right side
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .small_button("Copy")
                                    .on_hover_text("Copy message to clipboard")
                                    .clicked()
                                {
                                    message_to_copy = Some(format!(
                                        "[{}] [{}] {}",
                                        format_rel_time(msg.rel_time_s),
                                        msg.severity.label(),
                                        msg.message
                                    ));
                                }
                            });
                        });
                    });
                ui.separator();
            }
        });

    if let Some(text) = message_to_copy {
        ui.ctx().copy_text(text);
    }

    MessagesAction::None
}

fn render_stat_pill(ui: &mut Ui, text: &str, is_selected: bool, active_color: Color32, on_click: impl FnOnce()) {
    let fill = if is_selected {
        active_color
    } else {
        Color32::from_rgba_premultiplied(
            (active_color.r() as f32 * 0.15) as u8,
            (active_color.g() as f32 * 0.15) as u8,
            (active_color.b() as f32 * 0.15) as u8,
            255,
        )
    };

    let text_color = if is_selected {
        Color32::WHITE
    } else {
        active_color
    };

    let btn = egui::Button::new(RichText::new(text).size(12.0).strong().color(text_color))
        .fill(fill)
        .stroke(Stroke::new(1.0, active_color))
        .corner_radius(4.0)
        .min_size(Vec2::new(70.0, 24.0));

    if ui.add(btn).clicked() {
        on_click();
    }
}

fn render_empty_state(ui: &mut Ui, active_log_name: Option<&str>) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(RichText::new("Messages & Flight Errors").size(20.0).strong());
            ui.add_space(10.0);

            if let Some(name) = active_log_name {
                ui.label(
                    RichText::new(format!("No extracted messages found for '{}'", name))
                        .size(14.0)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            } else {
                ui.label(
                    RichText::new("Select a log from the sidebar to inspect its messages and errors.")
                        .size(14.0)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            }

            ui.add_space(16.0);
            ui.label(
                "Messages are extracted automatically from the following tables:\n\
                • MSG: Pre-Arm check failures, hardware status, mission events\n\
                • STATUSTEXT: MAVLink system warnings & errors with severity levels\n\
                • ERR: Subsystem hardware & sensor faults (Compass, Baro, RC, EKF)\n\
                • EV: Flight events (Armed, Disarmed, Takeoff, Fence breached)",
            );
            ui.add_space(20.0);
            ui.label(
                RichText::new("If this log hasn't been extracted yet, click 'Extract Log Tables' in the sidebar.")
                    .size(12.0)
                    .color(Color32::from_rgb(96, 165, 250)),
            );
        });
    });
}

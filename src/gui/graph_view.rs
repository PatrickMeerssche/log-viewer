use crate::models::{ChartType, LoadedTable};
use crate::plot_state::PlotState;
use egui::{Color32, CornerRadius, RichText, ScrollArea, Ui, Vec2};
use egui_plot::{CoordinatesFormatter, Corner, Legend, Line, LineStyle, MarkerShape, Plot, PlotPoints, Points};
use std::collections::HashSet;

pub fn render_graph_view(
    ui: &mut Ui,
    table: &LoadedTable,
    state: &mut PlotState,
) {
    state.update_series_if_dirty(table);

    ui.vertical(|ui| {
        // --- 1. TOP TOOLBAR ---
        egui::Frame::group(ui.style())
            .fill(ui.visuals().window_fill())
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // X-Axis Selector
                    ui.label(RichText::new("X-Axis:").strong().size(12.0));
                    let prev_x = state.selected_x.clone();
                    egui::ComboBox::from_id_salt("graph_x_select")
                        .selected_text(if state.selected_x == "__ROW_INDEX__" {
                            "Row Index (Sample #)"
                        } else {
                            &state.selected_x
                        })
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(state.selected_x == "__ROW_INDEX__", "Row Index (Sample #)")
                                .clicked()
                            {
                                state.selected_x = "__ROW_INDEX__".to_string();
                            }
                            for h in &table.headers {
                                let is_sel = state.selected_x == *h;
                                if ui.selectable_label(is_sel, h).clicked() {
                                    state.selected_x = h.clone();
                                }
                            }
                        });
                    if prev_x != state.selected_x {
                        state.is_dirty = true;
                        state.request_reset_bounds = true;
                    }

                    // Relative Time Toggle
                    if state.selected_x != "__ROW_INDEX__" {
                        if ui
                            .checkbox(&mut state.relative_time, "Relative Time (T - T₀)")
                            .on_hover_text("Offset timestamp so flight starts at 0.0s (converts us/ms to seconds)")
                            .changed()
                        {
                            state.is_dirty = true;
                            state.request_reset_bounds = true;
                        }
                    }

                    ui.separator();

                    // Group / Split Selector (only show columns with <= 12 unique values)
                    let valid_group_cols = find_valid_grouping_columns(table);
                    if !valid_group_cols.is_empty() {
                        ui.label(RichText::new("Split by:").strong().size(12.0));
                        let prev_group = state.group_by_col.clone();
                        egui::ComboBox::from_id_salt("graph_group_select")
                            .selected_text(state.group_by_col.as_deref().unwrap_or("(None)"))
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(state.group_by_col.is_none(), "(None)")
                                    .clicked()
                                {
                                    state.group_by_col = None;
                                }
                                for g in &valid_group_cols {
                                    if *g == state.selected_x {
                                        continue;
                                    }
                                    let is_sel = state.group_by_col.as_deref() == Some(g.as_str());
                                    if ui.selectable_label(is_sel, g).clicked() {
                                        state.group_by_col = Some(g.clone());
                                    }
                                }
                            });
                        if prev_group != state.group_by_col {
                            state.is_dirty = true;
                            state.request_reset_bounds = true;
                        }

                        ui.separator();
                    }

                    // Chart Style
                    ui.label(RichText::new("Style:").strong().size(12.0));
                    egui::ComboBox::from_id_salt("graph_type_select")
                        .selected_text(match state.chart_type {
                            ChartType::Line => "Line",
                            ChartType::Scatter => "Scatter (Dots)",
                            ChartType::Step => "Step",
                        })
                        .width(105.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.chart_type, ChartType::Line, "Line");
                            ui.selectable_value(&mut state.chart_type, ChartType::Scatter, "Scatter (Dots)");
                            ui.selectable_value(&mut state.chart_type, ChartType::Step, "Step");
                        });

                    if state.chart_type != ChartType::Scatter {
                        ui.label("Width:");
                        ui.add(egui::Slider::new(&mut state.line_width, 0.5..=4.0).show_value(false));
                    }

                    ui.separator();

                    // Normalization toggle
                    if ui
                        .checkbox(&mut state.normalize_y, "Normalize Y (0-100%)")
                        .on_hover_text("Scale each curve to 0-100% min-max to compare variables with different units (e.g. Volt vs Curr)")
                        .changed()
                    {
                        state.is_dirty = true;
                        state.request_reset_bounds = true;
                    }

                    ui.separator();

                    ui.checkbox(&mut state.show_legend, "Legend");

                    ui.separator();

                    // Points limit
                    egui::ComboBox::from_id_salt("graph_downsample_select")
                        .selected_text(match state.downsample_limit {
                            0 => "All pts".to_string(),
                            n => format!("{}k pts", n / 1000),
                        })
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.downsample_limit, 5_000, "5k pts");
                            ui.selectable_value(&mut state.downsample_limit, 10_000, "10k pts");
                            ui.selectable_value(&mut state.downsample_limit, 25_000, "25k pts");
                            ui.selectable_value(&mut state.downsample_limit, 50_000, "50k pts");
                            ui.selectable_value(&mut state.downsample_limit, 0, "All pts");
                        });

                    ui.separator();

                    // Auto bounds / Fit view button
                    if ui
                        .button(RichText::new("🎯 Fit View").strong().color(Color32::from_rgb(147, 197, 253)))
                        .on_hover_text("Reset zoom and camera bounds to fit all plotted points")
                        .clicked()
                    {
                        state.request_reset_bounds = true;
                    }

                    ui.separator();

                    // Export Image menu
                    ui.menu_button(RichText::new("📷 Export Plot").strong().color(Color32::from_rgb(110, 231, 183)), |ui| {
                        if ui
                            .button("📐 Save as Vector SVG (.svg)...")
                            .on_hover_text("High-resolution scalable vector graphic for papers, posters, and reports")
                            .clicked()
                        {
                            ui.close();
                            let palette = get_color_palette();
                            let svg_data = crate::plot_export::export_plot_to_svg(
                                &table.name,
                                &state.selected_x,
                                &state.cached_series,
                                &palette,
                                1920,
                                1080,
                            );
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&format!("{}_plot.svg", table.name))
                                .add_filter("SVG Image", &["svg"])
                                .save_file()
                            {
                                let _ = crate::plot_export::save_svg_file(&path, &svg_data);
                            }
                        }

                        if ui
                            .button("🖼️ Save as High-Res PNG (.png)...")
                            .on_hover_text("Export 1920x1080 high-definition raster image")
                            .clicked()
                        {
                            ui.close();
                            let palette = get_color_palette();
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&format!("{}_plot.png", table.name))
                                .add_filter("PNG Image", &["png"])
                                .save_file()
                            {
                                let _ = crate::plot_export::export_plot_to_png(
                                    &path,
                                    &table.name,
                                    &state.selected_x,
                                    &state.cached_series,
                                    &palette,
                                    1920,
                                    1080,
                                );
                            }
                        }
                    });
                });
            });

        ui.add_space(4.0);

        // Compute available space to extend to bottom and right edges of the screen
        let full_height = ui.available_height().max(300.0);
        let var_panel_width = state.var_panel_width.clamp(180.0, 500.0);

        // --- 2. MAIN AREA: LEFT VARIABLE PICKER + RIGHT PLOT CANVAS ---
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.set_min_height(full_height);
            ui.set_max_height(full_height);

            // LEFT PANEL: VARIABLE PICKER
            egui::Frame::group(ui.style())
                .fill(ui.visuals().window_fill())
                .corner_radius(CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.set_width(var_panel_width);
                    ui.set_min_height(full_height);
                    ui.set_max_height(full_height);

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("📈 Variables to Plot").strong().size(13.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("Clear").size(10.0)).clicked() {
                                    state.selected_y.clear();
                                    state.is_dirty = true;
                                    state.request_reset_bounds = true;
                                }
                                if ui.button(RichText::new("All").size(10.0)).clicked() {
                                    for stat in &table.stats {
                                        if stat.is_numeric && stat.name != state.selected_x {
                                            state.selected_y.insert(stat.name.clone());
                                        }
                                    }
                                    state.is_dirty = true;
                                    state.request_reset_bounds = true;
                                }
                            });
                        });

                        // Smart Preset Buttons based on table
                        render_smart_presets(ui, table, state);

                        ui.add(
                            egui::TextEdit::singleline(&mut state.column_search)
                                .hint_text("🔍 Search columns...")
                                .desired_width(ui.available_width()),
                        );

                        ui.separator();

                        let search_query = state.column_search.to_lowercase();
                        let palette = get_color_palette();

                        // Column checkboxes list fills remaining height
                        ScrollArea::vertical()
                            .id_salt("graph_var_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for stat in &table.stats {
                                    if !stat.is_numeric || stat.name == state.selected_x {
                                        continue;
                                    }
                                    if !search_query.is_empty() && !stat.name.to_lowercase().contains(&search_query) {
                                        continue;
                                    }

                                    let mut is_selected = state.selected_y.contains(&stat.name);
                                    let series_idx = state.cached_series.iter().position(|s| s.raw_y_name == stat.name);
                                    let series_color = series_idx.map(|idx| palette[idx % palette.len()]);

                                    ui.horizontal(|ui| {
                                        // Color swatch if active
                                        if let Some(col) = series_color {
                                            let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                                            ui.painter().rect_filled(rect, 2.0, col);
                                        } else {
                                            ui.add_space(10.0);
                                        }

                                        if ui.checkbox(&mut is_selected, &stat.name).changed() {
                                            if is_selected {
                                                state.selected_y.insert(stat.name.clone());
                                            } else {
                                                state.selected_y.remove(&stat.name);
                                            }
                                            state.is_dirty = true;
                                            state.request_reset_bounds = true;
                                        }

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Solo button: plot ONLY this column with 1 click
                                            if ui
                                                .button(RichText::new("👁 Solo").size(10.0))
                                                .on_hover_text("Plot ONLY this variable")
                                                .clicked()
                                            {
                                                state.selected_y.clear();
                                                state.selected_y.insert(stat.name.clone());
                                                state.is_dirty = true;
                                                state.request_reset_bounds = true;
                                            }
                                        });
                                    });

                                    // Display value range
                                    if let (Some(min), Some(max)) = (stat.min, stat.max) {
                                        ui.horizontal(|ui| {
                                            ui.add_space(18.0);
                                            ui.label(
                                                RichText::new(format!("range: {:.2} .. {:.2}", min, max))
                                                    .size(9.5)
                                                    .color(Color32::from_rgb(148, 163, 184)),
                                            );
                                        });
                                    }
                                }
                            });

                        // Active series summary
                        if !state.cached_series.is_empty() {
                            ui.separator();
                            ui.label(RichText::new(format!("Active: {} curves", state.cached_series.len())).size(11.0).color(Color32::GRAY));
                        }
                    });
                });

            // Splitter handle
            let (splitter_rect, splitter_resp) = ui.allocate_exact_size(Vec2::new(5.0, full_height), egui::Sense::click_and_drag());
            if splitter_resp.dragged() {
                state.var_panel_width = (state.var_panel_width + splitter_resp.drag_delta().x).clamp(180.0, 500.0);
            }
            if splitter_resp.hovered() || splitter_resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                ui.painter().rect_filled(splitter_rect, 2.0, Color32::from_rgb(59, 130, 246));
            }

            // RIGHT AREA: PLOT CANVAS
            let plot_width = ui.available_width().max(200.0);
            ui.vertical(|ui| {
                ui.set_width(plot_width);
                ui.set_min_height(full_height);
                ui.set_max_height(full_height);

                if state.selected_y.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading(RichText::new("📊 No Variables Selected").size(18.0).color(Color32::GRAY));
                            ui.add_space(6.0);
                            ui.label("Select variables from the list on the left to start graphing.");
                            ui.add_space(10.0);
                            if ui.button(RichText::new("⚡ Select First Numeric Column").size(13.0)).clicked() {
                                if let Some(first) = table.stats.iter().find(|s| s.is_numeric && s.name != state.selected_x) {
                                    state.selected_y.insert(first.name.clone());
                                    state.is_dirty = true;
                                    state.request_reset_bounds = true;
                                }
                            }
                        });
                    });
                    return;
                }

                let palette = get_color_palette();
                let is_rel = state.relative_time;

                let mut plot = Plot::new("telemetry_graph")
                    .width(plot_width)
                    .height(full_height)
                    .coordinates_formatter(
                        Corner::LeftBottom,
                        CoordinatesFormatter::new(move |point, _| {
                            if is_rel {
                                format!("Time (X): {:.3}s\nValue (Y): {:.4}", point.x, point.y)
                            } else {
                                format!("X: {:.4}\nY: {:.4}", point.x, point.y)
                            }
                        }),
                    )
                    .show_grid(state.show_grid);

                if state.show_legend {
                    plot = plot.legend(
                        Legend::default()
                            .position(Corner::RightTop)
                            .background_alpha(0.7),
                    );
                }

                plot.show(ui, |plot_ui| {
                    if state.request_reset_bounds {
                        plot_ui.set_auto_bounds(true);
                        state.request_reset_bounds = false;
                    }

                    for (idx, series) in state.cached_series.iter().enumerate() {
                        let color = palette[idx % palette.len()];
                        let points = PlotPoints::new(series.points.clone());

                        match state.chart_type {
                            ChartType::Line => {
                                let line = Line::new(&series.label, points)
                                    .color(color)
                                    .width(state.line_width);
                                plot_ui.line(line);
                            }
                            ChartType::Scatter => {
                                let pts = Points::new(&series.label, points)
                                    .color(color)
                                    .radius(state.line_width * 2.0)
                                    .shape(MarkerShape::Circle);
                                plot_ui.points(pts);
                            }
                            ChartType::Step => {
                                let line = Line::new(&series.label, points)
                                    .color(color)
                                    .width(state.line_width)
                                    .style(LineStyle::Solid);
                                plot_ui.line(line);
                            }
                        }
                    }
                });
            });
        });
    });
}

fn find_valid_grouping_columns(table: &LoadedTable) -> Vec<String> {
    let mut valid = Vec::new();
    for stat in &table.stats {
        if let Some(col_idx) = table.headers.iter().position(|h| h == &stat.name) {
            let mut unique = HashSet::new();
            for row in &table.rows {
                if let Some(val) = row.get(col_idx) {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        unique.insert(trimmed);
                        if unique.len() > 12 {
                            break;
                        }
                    }
                }
            }
            if unique.len() >= 2 && unique.len() <= 12 {
                valid.push(stat.name.clone());
            }
        }
    }
    valid
}

fn render_smart_presets(ui: &mut Ui, table: &LoadedTable, state: &mut PlotState) {
    let name_up = table.name.to_uppercase();

    if name_up.contains("BAT") {
        if ui.button(RichText::new("⚡ Curr & Volt").size(10.5)).clicked() {
            state.selected_y.clear();
            for candidate in &["Curr", "current_battery", "Volt", "Volt_ext", "voltages"] {
                if table.stats.iter().any(|s| s.is_numeric && s.name.eq_ignore_ascii_case(candidate)) {
                    state.selected_y.insert(candidate.to_string());
                }
            }
            state.normalize_y = true; // Auto-normalize since volts and amps have different units!
            state.is_dirty = true;
            state.request_reset_bounds = true;
        }
    } else if name_up.contains("ATT") {
        if ui.button(RichText::new("🔄 Roll, Pitch, Yaw").size(10.5)).clicked() {
            state.selected_y.clear();
            for candidate in &["Roll", "Pitch", "Yaw", "DesRoll", "DesPitch", "DesYaw"] {
                if table.stats.iter().any(|s| s.is_numeric && s.name.eq_ignore_ascii_case(candidate)) {
                    state.selected_y.insert(candidate.to_string());
                }
            }
            state.normalize_y = false;
            state.is_dirty = true;
            state.request_reset_bounds = true;
        }
    } else if name_up.contains("IMU") {
        ui.horizontal(|ui| {
            if ui.button(RichText::new("📈 Accel X/Y/Z").size(10.0)).clicked() {
                state.selected_y.clear();
                for candidate in &["AccX", "AccY", "AccZ", "xacc", "yacc", "zacc"] {
                    if table.stats.iter().any(|s| s.is_numeric && s.name.eq_ignore_ascii_case(candidate)) {
                        state.selected_y.insert(candidate.to_string());
                    }
                }
                state.is_dirty = true;
                state.request_reset_bounds = true;
            }
            if ui.button(RichText::new("🧭 Gyro X/Y/Z").size(10.0)).clicked() {
                state.selected_y.clear();
                for candidate in &["GyrX", "GyrY", "GyrZ", "xgyro", "ygyro", "zgyro"] {
                    if table.stats.iter().any(|s| s.is_numeric && s.name.eq_ignore_ascii_case(candidate)) {
                        state.selected_y.insert(candidate.to_string());
                    }
                }
                state.is_dirty = true;
                state.request_reset_bounds = true;
            }
        });
    } else if name_up.contains("CTUN") {
        if ui.button(RichText::new("📏 Alt & DesAlt").size(10.5)).clicked() {
            state.selected_y.clear();
            for candidate in &["Alt", "DAlt", "BAlt", "CRt", "DCRt"] {
                if table.stats.iter().any(|s| s.is_numeric && s.name.eq_ignore_ascii_case(candidate)) {
                    state.selected_y.insert(candidate.to_string());
                }
            }
            state.is_dirty = true;
            state.request_reset_bounds = true;
        }
    } else if name_up.contains("VIBE") {
        if ui.button(RichText::new("📳 Vibe X/Y/Z").size(10.5)).clicked() {
            state.selected_y.clear();
            for candidate in &["VibeX", "VibeY", "VibeZ", "Clip0", "Clip1"] {
                if table.stats.iter().any(|s| s.is_numeric && s.name.eq_ignore_ascii_case(candidate)) {
                    state.selected_y.insert(candidate.to_string());
                }
            }
            state.is_dirty = true;
            state.request_reset_bounds = true;
        }
    }
}

fn get_color_palette() -> [Color32; 10] {
    [
        Color32::from_rgb(59, 130, 246),   // Blue
        Color32::from_rgb(249, 115, 22),   // Orange
        Color32::from_rgb(16, 185, 129),   // Emerald Green
        Color32::from_rgb(236, 72, 153),   // Pink
        Color32::from_rgb(168, 85, 247),   // Purple
        Color32::from_rgb(234, 179, 8),    // Yellow
        Color32::from_rgb(14, 165, 233),   // Sky
        Color32::from_rgb(239, 68, 68),    // Red
        Color32::from_rgb(20, 184, 166),   // Teal
        Color32::from_rgb(163, 230, 53),   // Lime
    ]
}

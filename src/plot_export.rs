use crate::models::SeriesData;
use egui::Color32;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Generate high-resolution, infinitely scalable SVG of the current plot
pub fn export_plot_to_svg(
    table_name: &str,
    x_label: &str,
    series_list: &[SeriesData],
    palette: &[Color32],
    width: u32,
    height: u32,
) -> String {
    if series_list.is_empty() {
        return format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}"><text x="50%" y="50%" fill="gray" text-anchor="middle">No data to plot</text></svg>"#,
            width, height
        );
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for s in series_list {
        for &[x, y] in &s.points {
            if x.is_finite() {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
            if y.is_finite() {
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }

    if !min_x.is_finite() || !max_x.is_finite() || min_x >= max_x {
        min_x = 0.0;
        max_x = 1.0;
    }
    if !min_y.is_finite() || !max_y.is_finite() || min_y >= max_y {
        min_y = 0.0;
        max_y = 1.0;
    }

    // Add 5% padding to Y range
    let y_span = max_y - min_y;
    let pad_y = if y_span > 0.0 { y_span * 0.05 } else { 1.0 };
    min_y -= pad_y;
    max_y += pad_y;

    let margin_left = 90.0f64;
    let margin_right = 40.0f64;
    let margin_top = 70.0f64;
    let margin_bottom = 60.0f64;

    let plot_w = width as f64 - margin_left - margin_right;
    let plot_h = height as f64 - margin_top - margin_bottom;

    let mut svg = String::with_capacity(64 * 1024);
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}" style="background-color: #12151c; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;">
"##,
        width, height, width, height
    ));

    // Title
    let title = format!("{}: Telemetry Data vs {}", table_name, x_label);
    svg.push_str(&format!(
        r##"  <text x="{}" y="38" fill="#f8fafc" font-size="20" font-weight="700">{}</text>
"##,
        margin_left, title
    ));

    // Plot Background
    svg.push_str(&format!(
        r##"  <rect x="{}" y="{}" width="{}" height="{}" fill="#181c26" stroke="#334155" stroke-width="1.5" rx="6" />
"##,
        margin_left, margin_top, plot_w, plot_h
    ));

    // Horizontal Grid Lines & Y Axis Ticks (6 steps)
    let y_steps = 6;
    for i in 0..=y_steps {
        let frac = i as f64 / y_steps as f64;
        let y_val = min_y + frac * (max_y - min_y);
        let py = margin_top + plot_h - frac * plot_h;

        svg.push_str(&format!(
            r##"  <line x1="{}" y1="{:.1}" x2="{}" y2="{:.1}" stroke="#2d3748" stroke-width="1" stroke-dasharray="4,4" />
  <text x="{:.1}" y="{:.1}" fill="#94a3b8" font-size="12" text-anchor="end" font-family="monospace">{:.2}</text>
"##,
            margin_left, py, margin_left + plot_w, py,
            margin_left - 10.0, py + 4.0, y_val
        ));
    }

    // Vertical Grid Lines & X Axis Ticks (8 steps)
    let x_steps = 8;
    for i in 0..=x_steps {
        let frac = i as f64 / x_steps as f64;
        let x_val = min_x + frac * (max_x - min_x);
        let px = margin_left + frac * plot_w;

        svg.push_str(&format!(
            r##"  <line x1="{:.1}" y1="{}" x2="{:.1}" y2="{}" stroke="#2d3748" stroke-width="1" stroke-dasharray="4,4" />
  <text x="{:.1}" y="{:.1}" fill="#94a3b8" font-size="12" text-anchor="middle" font-family="monospace">{:.1}s</text>
"##,
            px, margin_top, px, margin_top + plot_h,
            px, margin_top + plot_h + 20.0, x_val
        ));
    }

    // X Axis Title
    svg.push_str(&format!(
        r##"  <text x="{:.1}" y="{:.1}" fill="#cbd5e1" font-size="13" font-weight="600" text-anchor="middle">{}</text>
"##,
        margin_left + plot_w / 2.0, margin_top + plot_h + 45.0, x_label
    ));

    // Clip path for series
    svg.push_str(&format!(
        r##"  <defs>
    <clipPath id="plot-clip">
      <rect x="{}" y="{}" width="{}" height="{}" rx="4" />
    </clipPath>
  </defs>
  <g clip-path="url(#plot-clip)">
"##,
        margin_left, margin_top, plot_w, plot_h
    ));

    // Data curves
    for (idx, series) in series_list.iter().enumerate() {
        let color = palette[idx % palette.len()];
        let color_hex = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());

        let mut points_str = String::new();
        for &[x, y] in &series.points {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let px = margin_left + ((x - min_x) / (max_x - min_x)) * plot_w;
            let py = margin_top + plot_h - ((y - min_y) / (max_y - min_y)) * plot_h;
            points_str.push_str(&format!("{:.1},{:.1} ", px, py));
        }

        if !points_str.is_empty() {
            svg.push_str(&format!(
                r##"    <polyline points="{}" fill="none" stroke="{}" stroke-width="2.0" stroke-linejoin="round" stroke-linecap="round" />
"##,
                points_str.trim_end(), color_hex
            ));
        }
    }
    svg.push_str("  </g>\n");

    // Interactive Legend in Top Right
    let legend_x = margin_left + plot_w - 180.0;
    let mut legend_y = margin_top + 16.0;

    for (idx, series) in series_list.iter().take(6).enumerate() {
        let color = palette[idx % palette.len()];
        let color_hex = format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b());

        svg.push_str(&format!(
            r##"  <rect x="{:.1}" y="{:.1}" width="12" height="12" fill="{}" rx="2" />
  <text x="{:.1}" y="{:.1}" fill="#f1f5f9" font-size="12" font-weight="600">{}</text>
"##,
            legend_x, legend_y - 10.0, color_hex,
            legend_x + 18.0, legend_y, series.label
        ));
        legend_y += 18.0;
    }

    svg.push_str("</svg>\n");
    svg
}

/// Save SVG string directly to a file
pub fn save_svg_file(path: &Path, svg_content: &str) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(svg_content.as_bytes())
}

/// High-resolution PNG rasterizer (1920x1080 default)
pub fn export_plot_to_png(
    path: &Path,
    _table_name: &str,
    _x_label: &str,
    series_list: &[SeriesData],
    palette: &[Color32],
    width: u32,
    height: u32,
) -> Result<(), String> {
    let mut img = image::RgbaImage::from_pixel(width, height, image::Rgba([18, 21, 28, 255]));

    if series_list.is_empty() {
        img.save(path).map_err(|e| format!("Failed to save PNG: {}", e))?;
        return Ok(());
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for s in series_list {
        for &[x, y] in &s.points {
            if x.is_finite() {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
            if y.is_finite() {
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }

    if !min_x.is_finite() || !max_x.is_finite() || min_x >= max_x {
        min_x = 0.0;
        max_x = 1.0;
    }
    if !min_y.is_finite() || !max_y.is_finite() || min_y >= max_y {
        min_y = 0.0;
        max_y = 1.0;
    }

    let y_span = max_y - min_y;
    let pad_y = if y_span > 0.0 { y_span * 0.05 } else { 1.0 };
    min_y -= pad_y;
    max_y += pad_y;

    let margin_left = 90.0f64;
    let margin_right = 40.0f64;
    let margin_top = 70.0f64;
    let margin_bottom = 60.0f64;

    let plot_w = width as f64 - margin_left - margin_right;
    let plot_h = height as f64 - margin_top - margin_bottom;

    // Draw inner plot area background [24, 28, 38]
    let plot_bg = image::Rgba([24, 28, 38, 255]);
    for y in margin_top as u32..((margin_top + plot_h) as u32).min(height) {
        for x in margin_left as u32..((margin_left + plot_w) as u32).min(width) {
            img.put_pixel(x, y, plot_bg);
        }
    }

    // Draw grid lines
    let grid_color = image::Rgba([45, 55, 72, 255]);
    let y_steps = 6;
    for i in 0..=y_steps {
        let frac = i as f64 / y_steps as f64;
        let py = (margin_top + plot_h - frac * plot_h) as i32;
        if py >= 0 && (py as u32) < height {
            for x in (margin_left as u32)..((margin_left + plot_w) as u32).min(width) {
                if (x / 4) % 2 == 0 {
                    img.put_pixel(x, py as u32, grid_color);
                }
            }
        }
    }

    let x_steps = 8;
    for i in 0..=x_steps {
        let frac = i as f64 / x_steps as f64;
        let px = (margin_left + frac * plot_w) as i32;
        if px >= 0 && (px as u32) < width {
            for y in (margin_top as u32)..((margin_top + plot_h) as u32).min(height) {
                if (y / 4) % 2 == 0 {
                    img.put_pixel(px as u32, y, grid_color);
                }
            }
        }
    }

    // Border around plot area
    let border_color = image::Rgba([51, 65, 85, 255]);
    for x in (margin_left as u32)..=((margin_left + plot_w) as u32).min(width - 1) {
        img.put_pixel(x, margin_top as u32, border_color);
        img.put_pixel(x, (margin_top + plot_h) as u32, border_color);
    }
    for y in (margin_top as u32)..=((margin_top + plot_h) as u32).min(height - 1) {
        img.put_pixel(margin_left as u32, y, border_color);
        img.put_pixel((margin_left + plot_w) as u32, y, border_color);
    }

    // Draw Series Lines (with 2px thickness)
    for (idx, series) in series_list.iter().enumerate() {
        let color = palette[idx % palette.len()];
        let rgba = image::Rgba([color.r(), color.g(), color.b(), 255]);

        let mut prev_pt: Option<(i32, i32)> = None;
        for &[x, y] in &series.points {
            if !x.is_finite() || !y.is_finite() {
                prev_pt = None;
                continue;
            }

            let px = (margin_left + ((x - min_x) / (max_x - min_x)) * plot_w) as i32;
            let py = (margin_top + plot_h - ((y - min_y) / (max_y - min_y)) * plot_h) as i32;

            if let Some((x0, y0)) = prev_pt {
                draw_thick_line_segment(&mut img, x0, y0, px, py, rgba, margin_left as i32, margin_top as i32, (margin_left + plot_w) as i32, (margin_top + plot_h) as i32);
            }
            prev_pt = Some((px, py));
        }
    }

    img.save(path).map_err(|e| format!("Failed to save PNG: {}", e))?;
    Ok(())
}

fn draw_thick_line_segment(
    img: &mut image::RgbaImage,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: image::Rgba<u8>,
    clip_min_x: i32,
    clip_min_y: i32,
    clip_max_x: i32,
    clip_max_y: i32,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let w = img.width() as i32;
    let h = img.height() as i32;

    loop {
        for ox in -1..=1 {
            for oy in -1..=1 {
                let px = x0 + ox;
                let py = y0 + oy;
                if px >= clip_min_x && px <= clip_max_x && py >= clip_min_y && py <= clip_max_y && px >= 0 && px < w && py >= 0 && py < h {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }

        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_plot_to_svg_empty() {
        let svg = export_plot_to_svg("ATT", "TimeUS", &[], &[], 800, 600);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("No data to plot"));
    }

    #[test]
    fn test_export_plot_to_svg_with_data() {
        let series = vec![
            SeriesData {
                label: "Roll".to_string(),
                raw_y_name: "Roll".to_string(),
                points: vec![[0.0, 10.0], [1.0, 20.0], [2.0, 15.0]],
                min_y: 10.0,
                max_y: 20.0,
            },
            SeriesData {
                label: "Pitch".to_string(),
                raw_y_name: "Pitch".to_string(),
                points: vec![[0.0, -5.0], [1.0, 5.0], [2.0, 0.0]],
                min_y: -5.0,
                max_y: 5.0,
            },
        ];
        let palette = vec![Color32::from_rgb(255, 0, 0), Color32::from_rgb(0, 255, 0)];
        let svg = export_plot_to_svg("ATT", "TimeUS", &series, &palette, 1200, 800);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("ATT: Telemetry Data vs TimeUS"));
        assert!(svg.contains("polyline"));
        assert!(svg.contains("Roll"));
        assert!(svg.contains("Pitch"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_export_plot_to_png() {
        let temp_dir = std::env::temp_dir();
        let test_png = temp_dir.join("test_plot_export.png");

        let series = vec![SeriesData {
            label: "Alt".to_string(),
            raw_y_name: "Alt".to_string(),
            points: vec![[0.0, 100.0], [1.0, 150.0], [2.0, 200.0]],
            min_y: 100.0,
            max_y: 200.0,
        }];
        let palette = vec![Color32::from_rgb(56, 189, 248)];

        let res = export_plot_to_png(
            &test_png,
            "GPS",
            "TimeUS",
            &series,
            &palette,
            640,
            480,
        );
        assert!(res.is_ok());
        assert!(test_png.exists());
        let metadata = std::fs::metadata(&test_png).unwrap();
        assert!(metadata.len() > 100);
        let _ = std::fs::remove_file(test_png);
    }
}


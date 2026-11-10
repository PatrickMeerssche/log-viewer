mod app;
mod csv_loader;
mod gui;
mod log_scanner;
mod messages_loader;
mod models;
mod plot_state;
mod web_server;

use app::TelemetryVisualizerApp;
use std::env;
use std::path::PathBuf;

fn print_help() {
    println!(
        r#"
Log & Telemetry Visualizer (Rust)

USAGE:
    log_visualizer [OPTIONS]

OPTIONS:
    -w, --web           Start web dashboard mode and open in browser
    -p, --port <PORT>   Web dashboard port (default: 8080)
    --logs <DIR>        Path to logs directory (default: ./logs)
    --csv <DIR>         Path to csv output directory (default: ./csv)
    --log <NAME>        Select log file on startup (e.g. log_13)
    --tab <TAB>         Select initial tab (graph, table, stats, messages)
    -h, --help          Print this help message
"#
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let mut web_mode = false;
    let mut port: u16 = 8080;
    let mut logs_dir = PathBuf::from("./logs");
    let mut csv_dir = PathBuf::from("./csv");
    let mut script_path = PathBuf::from("./old scripts/log_to_csv.py");
    if !script_path.exists() {
        script_path = PathBuf::from("./log_to_csv.py");
    }
    let mut initial_tab = None;
    let mut initial_log = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-w" | "--web" => {
                web_mode = true;
            }
            "-p" | "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(8080);
                    i += 1;
                }
            }
            "--logs" => {
                if i + 1 < args.len() {
                    logs_dir = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            "--csv" => {
                if i + 1 < args.len() {
                    csv_dir = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            "--tab" => {
                if i + 1 < args.len() {
                    let t = args[i + 1].to_lowercase();
                    initial_tab = match t.as_str() {
                        "messages" | "msg" | "errors" => Some(crate::app::ActiveTab::Messages),
                        "table" => Some(crate::app::ActiveTab::Table),
                        "stats" => Some(crate::app::ActiveTab::Stats),
                        "details" => Some(crate::app::ActiveTab::Details),
                        _ => Some(crate::app::ActiveTab::Graph),
                    };
                    i += 1;
                }
            }
            "--log" => {
                if i + 1 < args.len() {
                    initial_log = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // Ensure directories exist
    let _ = std::fs::create_dir_all(&logs_dir);
    let _ = std::fs::create_dir_all(&csv_dir);

    // Detect if display is available
    let has_display = env::var("DISPLAY").is_ok() || env::var("WAYLAND_DISPLAY").is_ok();

    if web_mode || !has_display {
        if !has_display && !web_mode {
            println!("ℹ️ No graphical display detected ($DISPLAY unset). Falling back to Web Dashboard.");
        }

        println!("🚀 Starting Telemetry & Log Visualizer Web Server...");
        let handle = web_server::start_web_server(logs_dir, csv_dir, script_path, port)?;
        let url = format!("http://127.0.0.1:{}", handle.port);
        println!("✨ Web Dashboard is running at: {}", url);
        println!("Opening in browser... (Press Ctrl+C to stop)");

        let _ = webbrowser::open(&url);

        // Keep running until Ctrl+C
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    } else {
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Log & Telemetry Visualizer")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([800.0, 600.0]),
            ..Default::default()
        };

        eframe::run_native(
            "Log & Telemetry Visualizer",
            native_options,
            Box::new(move |_cc| {
                let app = TelemetryVisualizerApp::new(logs_dir, csv_dir, script_path)
                    .with_initial(initial_log, initial_tab);
                Ok(Box::new(app))
            }),
        )
        .map_err(|e| format!("GUI launch failed: {}", e))?;
    }

    Ok(())
}

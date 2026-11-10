# ✈️ Telemetry & Flight Log Visualizer (Rust)

A fast, interactive Rust desktop & web application to scan, convert, inspect, and graph ArduPilot (`.bin`), MAVLink telemetry (`.tlog`), and CSV logs from `./logs`.

Built with **egui**, **eframe**, and **egui_plot** for native 60 FPS hardware-accelerated desktop visualization, with an embedded modern **Web Dashboard** powered by **Plotly.js** and **tiny_http**.

---

## ✨ Features

- **Automatic Log Discovery**:
  - Scans `./logs` for all `.bin`, `.tlog`, `.log`, and `.csv` files.
  - Live search/filtering of log files by name.
  - Shows file size, extension, and table conversion status badges.
- **One-Click Log-to-Tables Converter**:
  - Convert any `.bin` or `.tlog` log into individual message tables with a single click.
  - "Extract All" batch conversion button.
  - Runs in a background thread so the user interface never freezes.
  - Displays converter terminal output in the *Log Details* console.
- **Interactive Multi-Series Plotter (`egui_plot`)**:
  - Choose any column for the **X-Axis** (auto-detects `_timestamp`, `TimeUS`, or row index).
  - Choose one or multiple columns for the **Y-Axis** (e.g., `Roll`, `Pitch`, `Yaw` or `Curr`, `Volt`).
  - **Group / Split by Column**: Auto-detects instance identifiers like `Inst` or `id` (e.g. from battery tables) and splits the data into distinct colored series: `Curr (Inst=0)`, `Curr (Inst=1)`!
  - Chart types: **Line**, **Scatter (Dots)**, and **Step**.
  - Dynamic downsampling options (`5,000`, `10,000`, `25,000`, or full resolution) for smooth rendering of massive logs.
  - Real-time zoom (mouse wheel / drag), pan (right drag), auto bounds reset, and coordinate cursor tooltips.
  - Toggleable interactive legend.
- **Full Table Inspector**:
  - Browse every row and column of any extracted table.
  - Search filter across all table cells.
  - Sort by any column (numeric or alphabetical) with header click.
  - Paginated table with configurable page sizes (25, 50, 100, 250, 500, 1000).
  - "💾 Export Filtered CSV" button.
- **Column Statistics**:
  - Quick statistical breakdown for all columns: min, max, mean, standard deviation, count, and null counts.
  - One-click `📈 Plot` shortcut to jump straight to graphing that column.
- **🚨 Flight Messages & Errors Viewer**:
  - **Pre-Arm Failures & Critical Warnings**: Instantly detects and highlights pre-arm check failures (e.g. `PreArm: RC not found`, `PreArm: Rangefinder 1: Not Detected`, `PreArm: Gyros inconsistent`), battery warnings, and AHRS failures.
  - **Subsystem Error Decoding**: Automatically decodes hardware errors from the `ERR` table into human-readable subsystems (Radio Control, Compass, Barometer, Inertial Nav, EKF, FailSafes, GPS) and status (fault detected vs error cleared).
  - **Flight Event Tracking**: Automatically decodes flight events from the `EV` table (Armed, Disarmed, Auto-Armed, Takeoff, Land Complete, Fence Breached, Mode Changes).
  - **MAVLink Status Texts**: Parses severity levels (Emergency, Critical, Error, Warning, Notice, Info) from `STATUSTEXT` logs.
  - **Severity Badges & 1-Click Filters**: Interactive badges for `All`, `🚨 Errors`, `⚠️ Warnings`, `🚀 Events`, and `ℹ️ Info` to isolate issues in seconds.
  - **Live Search & Copy**: Filter across all messages, error codes, and subsystems in real time. 1-click `📋 Copy` per row or `📋 Copy All Filtered` for incident reporting.
  - **Tab Alert Highlighting**: Tab button highlights error counts with red alerts (e.g. `🚨 Messages & Errors (109)`).
- **Dual Mode (Desktop GUI & Web Dashboard)**:
  - **Desktop GUI**: Native immediate-mode GUI window.
  - **Web Dashboard**: Click "🌐 Web Dashboard" or run with `--web` to launch the browser UI with Plotly.js charts and web messages viewer.
  - Automatic fallback to Web Dashboard if no graphical display (`$DISPLAY`) is available.

---

## 📋 Requirements & Prerequisites

### 1. Rust Compiler (Required)
Install the official Rust toolchain via [rustup.rs](https://rustup.rs/):
```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
On **Windows**, download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs/).

### 2. Platform GUI Libraries (For Native Desktop GUI mode)

- **Ubuntu / Debian / Raspberry Pi OS**:
  ```bash
  sudo apt update
  sudo apt install -y build-essential pkg-config libx11-dev libxcb-render0-dev \
      libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libfontconfig1-dev
  ```
- **Fedora / RHEL**:
  ```bash
  sudo dnf install -y gcc gcc-c++ pkgconfig libX11-devel libxcb-devel libxkbcommon-devel fontconfig-devel
  ```
- **Arch Linux / Manjaro**:
  ```bash
  sudo pacman -S base-devel libx11 libxcb libxkbcommon fontconfig
  ```
- **Windows**:
  - Microsoft Visual Studio C++ Build Tools (MSVC) or MinGW toolchain (prompted by `rustup-init.exe`).
  - No additional external packages needed; hardware acceleration runs via native DirectX/Vulkan/WGPU.
- **macOS**:
  - Xcode Command Line Tools: `xcode-select --install`.

> [!NOTE]
> If running on a headless server without a desktop GUI (`$DISPLAY` unset), the app automatically starts the **Web Dashboard** on port 8080.

### 3. Python 3 & pymavlink (Optional — for raw `.bin` / `.tlog` extraction)
The application includes a background Python converter (`old scripts/log_to_csv.py`) for decoding binary ArduPilot `.bin` and MAVLink `.tlog` files into CSV tables.
```bash
pip install pymavlink
```
*(If you are only visualizing existing CSV files or pre-extracted tables, Python is not needed).*

---

## 🚀 Quick Start

### 1. Interactive Launchers (Recommended)

Simply press **Enter** for each prompt to accept default settings (Native GUI, `./logs`, `./csv`):

#### 🐧 On Linux, macOS, WSL, or Git Bash:
```bash
./log-viewer
```

#### 🪟 On Windows:
- **Command Prompt (`cmd.exe`)**:
  ```cmd
  log-viewer.bat
  ```
- **PowerShell**:
  ```powershell
  .\log-viewer.bat
  # or
  .\log-viewer.ps1
  ```
- **File Explorer**:
  Double-click `log-viewer.bat` to launch interactively.

---

### 2. Command-Line Flag Passthrough

Both `./log-viewer` and `log-viewer.bat` forward command-line arguments directly:

```bash
# Launch directly into Web Dashboard mode on port 8080:
./log-viewer --web
log-viewer.bat --web

# Open directly to the Messages & Errors tab for a specific log:
./log-viewer --tab messages --log log_13_2026-10-27-11-28-20.bin
log-viewer.bat --tab messages --log log_13_2026-10-27-11-28-20.bin

# Specify custom log and CSV directories:
./log-viewer --logs /path/to/my_logs --csv /path/to/my_csv
log-viewer.bat --logs C:\my_logs --csv C:\my_csv
```

---

### 3. Run Directly via Cargo

```bash
cargo run --release
# or
cargo run --release -- --web --port 8080
```

---

### 4. Command-Line Options

```bash
./target/release/log_visualizer --help
# (On Windows: .\target\release\log_visualizer.exe --help)
```

```
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
```

---

## 📂 Project Architecture

```
log-viewer/
├── Cargo.toml
├── logs/                   # Directory for flight logs (.bin, .tlog, .csv)
├── csv/                    # Extracted CSV tables organized by log stem
├── log-viewer              # Interactive launcher for Linux / macOS / Git Bash / WSL
├── log-viewer.bat          # Interactive launcher for Windows (Command Prompt / Explorer)
├── log-viewer.ps1          # Interactive launcher for Windows PowerShell
├── old scripts/            # Legacy standalone Python scripts
│   ├── log_to_csv.py       # ArduPilot & MAVLink parser script
│   ├── plot_bat_current.py # Standalone battery current plotter script
│   └── plot_battery_current.py # Standalone battery status plotter script
└── src/
    ├── main.rs             # CLI parsing & GUI/Web runner
    ├── app.rs              # Main eframe application loop & state
    ├── models.rs           # Core data structures (LogEntry, TableMeta, LoadedTable)
    ├── log_scanner.rs      # Discovery & asynchronous conversion runner
    ├── messages_loader.rs  # Parser for MSG, STATUSTEXT, ERR, EV tables & decoders
    ├── csv_loader.rs       # Fast CSV parser, stats calculator & series extractor
    ├── plot_state.rs       # Plot configurations, series caching & downsampling
    ├── web_server.rs       # Embedded tiny_http server & Plotly.js dashboard
    └── gui/
        ├── mod.rs
        ├── sidebar.rs      # Log & Table tree navigation
        ├── graph_view.rs   # egui_plot interactive charting canvas
        ├── table_view.rs   # Sortable, searchable paginated data table
        ├── stats_view.rs   # Summary stats per column
        ├── messages_view.rs# Messages, flight events, warnings & errors timeline
        └── details_view.rs # Log metadata, export & conversion console
```

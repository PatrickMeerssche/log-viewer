# Telemetry & Flight Log Visualizer

[![CI](https://github.com/PatrickMeerssche/log-viewer/actions/workflows/ci.yml/badge.svg)](https://github.com/PatrickMeerssche/log-viewer/actions/workflows/ci.yml)
[![GitHub release](https://img.shields.io/github/v/release/PatrickMeerssche/log-viewer?color=22c55e&label=latest%20release)](https://github.com/PatrickMeerssche/log-viewer/releases)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-blue)](https://github.com/PatrickMeerssche/log-viewer)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

A fast, interactive cross-platform Rust desktop & web application to scan, convert, inspect, and graph ArduPilot (`.bin`), MAVLink telemetry (`.tlog`), and CSV logs from `./logs`.

---

## Pre-compiled Standalone Releases (No Rust Required!)

If you just want to run the visualizer without installing Rust or Cargo:

1. Head over to the **[GitHub Releases Page](https://github.com/PatrickMeerssche/log-viewer/releases)**.
2. Download the pre-built package for your operating system:
   - **Windows**: `log-viewer-windows-x86_64.zip`
   - **Linux**: `log-viewer-linux-x86_64.tar.gz`
3. Extract the archive.
4. Run `./log-viewer` (Linux) or double-click `log-viewer.bat` (Windows)!

---

## Export Plots to High-Resolution Images

Export publication-ready graphs directly from both Desktop and Web interfaces:
- **Vector SVG (`.svg`)**: Crisp, infinitely scalable vector graphics for reports, papers, and presentations with grid lines, axes labels, and legend.
- **High-Resolution PNG (`.png`)**: Standalone, lossless raster image rendering of the active plot.
- **Desktop GUI**: Click **Save as** in the graph toolbar and select **SVG** or **PNG**.
- **Web Dashboard**: Click **SVG** or **PNG** under **Save as:** right next to the chart controls to instantly download the graph.

---

## Requirements & Prerequisites

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

## Quick Start

### 1. Interactive Launchers (Recommended)

Simply press **Enter** for each prompt to accept default settings (Native GUI, `./logs`, `./csv`):

#### On Linux, macOS, WSL, or Git Bash:

```bash
./log-viewer
```

#### On Windows:

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

## Project Architecture

```
log-viewer/
├── .github/
│   └── workflows/
│       ├── ci.yml              # Automated cross-platform CI (Ubuntu & Windows)
│       └── release.yml         # Automated GitHub Release packaging (.zip & .tar.gz)
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
    ├── plot_export.rs      # High-res SVG & PNG graph export engine
    ├── web_server.rs       # Embedded tiny_http server & Plotly.js dashboard
    └── gui/
        ├── mod.rs
        ├── sidebar.rs      # Log & Table tree navigation
        ├── graph_view.rs   # egui_plot interactive charting canvas & image exporter
        ├── table_view.rs   # Sortable, searchable paginated data table
        ├── stats_view.rs   # Summary stats per column
        ├── messages_view.rs# Messages, flight events, warnings & errors timeline
        └── details_view.rs # Log metadata, export & conversion console
```

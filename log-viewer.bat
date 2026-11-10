@echo off
setlocal enabledelayedexpansion

:: Change to script's directory
cd /d "%~dp0"

:: 1. Locate or build the executable
set "BIN="
if exist ".\log_visualizer.exe" (
    set "BIN=.\log_visualizer.exe"
) else if exist ".\target\release\log_visualizer.exe" (
    set "BIN=.\target\release\log_visualizer.exe"
) else if exist ".\target\debug\log_visualizer.exe" (
    set "BIN=.\target\debug\log_visualizer.exe"
) else (
    echo [INFO] Building log_visualizer binary (first run)...
    where cargo >nul 2>nul
    if %ERRORLEVEL% neq 0 (
        echo [ERROR] Cargo / Rust is not found in PATH!
        echo Please install Rust from https://rustup.rs/ and try again.
        pause
        exit /b 1
    )
    cargo build --release
    if exist ".\target\release\log_visualizer.exe" (
        set "BIN=.\target\release\log_visualizer.exe"
    ) else (
        echo [ERROR] Build failed or binary not found.
        pause
        exit /b 1
    )
)

:: 2. If command-line arguments are passed, pass them directly to the binary
if not "%~1"=="" (
    "%BIN%" %*
    exit /b %ERRORLEVEL%
)

:: 3. Interactive CLI Configuration
echo ======================================================
echo    Telemetry ^& Flight Log Visualizer Launcher
echo ======================================================
echo Press [Enter] to accept the default for any setting.
echo.

:: Question 1: Interface Mode
echo 1. Interface Mode:
echo    [1] Native Desktop GUI (default)
echo    [2] Web Dashboard (Browser)
set /p "MODE_INPUT=Select mode [1/2, default: 1]: "
set "WEB_FLAG="
if /i "!MODE_INPUT!"=="2" set "WEB_FLAG=--web"
if /i "!MODE_INPUT!"=="web" set "WEB_FLAG=--web"
if /i "!MODE_INPUT!"=="w" set "WEB_FLAG=--web"

if defined WEB_FLAG (
    echo Selected: Web Dashboard Mode
) else (
    echo Selected: Native Desktop GUI Mode
)
echo.

:: Question 2: Logs Directory
echo 2. Logs Directory:
set /p "LOGS_INPUT=Enter logs folder [default: .\logs]: "
if "!LOGS_INPUT!"=="" (
    set "LOGS_DIR=.\logs"
) else (
    set "LOGS_DIR=!LOGS_INPUT!"
)
echo Selected logs dir: !LOGS_DIR!
echo.

:: Question 3: CSV Output Directory
echo 3. CSV Output Directory:
set /p "CSV_INPUT=Enter CSV folder [default: .\csv]: "
if "!CSV_INPUT!"=="" (
    set "CSV_DIR=.\csv"
) else (
    set "CSV_DIR=!CSV_INPUT!"
)
echo Selected CSV dir: !CSV_DIR!
echo.

:: Question 4: Port (only asked in Web mode)
set "PORT_FLAG="
if defined WEB_FLAG (
    echo 4. Web Server Port:
    set /p "PORT_INPUT=Enter port number [default: 8080]: "
    if "!PORT_INPUT!"=="" (
        set "PORT_VAL=8080"
    ) else (
        set "PORT_VAL=!PORT_INPUT!"
    )
    set "PORT_FLAG=--port !PORT_VAL!"
    echo Selected port: !PORT_VAL!
    echo.
)

:: Launch the application
echo ------------------------------------------------------
echo Launching: !BIN! --logs "!LOGS_DIR!" --csv "!CSV_DIR!" !WEB_FLAG! !PORT_FLAG!
echo ------------------------------------------------------
echo.

"%BIN%" --logs "!LOGS_DIR!" --csv "!CSV_DIR!" !WEB_FLAG! !PORT_FLAG!
set "EXIT_CODE=%ERRORLEVEL%"

if %EXIT_CODE% neq 0 (
    echo.
    echo Application exited with code %EXIT_CODE%.
    pause
)

exit /b %EXIT_CODE%

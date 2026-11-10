<#
.SYNOPSIS
    Cross-platform PowerShell launcher for Telemetry & Flight Log Visualizer.
.DESCRIPTION
    Runs on Windows PowerShell 5.1 and PowerShell Core (pwsh) on Windows, Linux, and macOS.
#>

[CmdletBinding()]
param(
    [switch]$Web,
    [string]$Logs = "",
    [string]$Csv = "",
    [int]$Port = 0,
    [string]$Log = "",
    [string]$Tab = ""
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

# Determine executable name based on OS
$ExeName = if ($IsWindows -or ($env:OS -like "*Windows*")) { "log_visualizer.exe" } else { "log_visualizer" }
$BinPath = Join-Path $ScriptDir $ExeName

if (-not (Test-Path $BinPath)) {
    $ReleaseBin = Join-Path $ScriptDir "target\release\$ExeName"
    if (Test-Path $ReleaseBin) {
        $BinPath = $ReleaseBin
    } else {
        $DebugBin = Join-Path $ScriptDir "target\debug\$ExeName"
        if (Test-Path $DebugBin) {
            $BinPath = $DebugBin
        } else {
            Write-Host "[INFO] Building log_visualizer binary..." -ForegroundColor Cyan
            if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
                Write-Error "Cargo / Rust is not found in PATH! Please install Rust from https://rustup.rs/"
                exit 1
            }
            cargo build --release
            if (Test-Path $ReleaseBin) {
                $BinPath = $ReleaseBin
            } else {
                Write-Error "Build failed or binary not found at $ReleaseBin"
                exit 1
            }
        }
    }
}

# If parameters or arguments were passed, forward directly
$PassedDirectArgs = $args
if ($PSBoundParameters.Count -gt 0 -or $PassedDirectArgs.Count -gt 0) {
    $RunArgs = @()
    if ($Logs) { $RunArgs += @("--logs", $Logs) }
    if ($Csv) { $RunArgs += @("--csv", $Csv) }
    if ($Web) { $RunArgs += @("--web") }
    if ($Port -gt 0) { $RunArgs += @("--port", $Port.ToString()) }
    if ($Log) { $RunArgs += @("--log", $Log) }
    if ($Tab) { $RunArgs += @("--tab", $Tab) }
    $RunArgs += $PassedDirectArgs

    & $BinPath @RunArgs
    exit $LASTEXITCODE
}

# Interactive CLI configuration when run with no arguments
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "      Telemetry & Flight Log Visualizer Launcher      " -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "Press [Enter] to accept the default for any setting.`n"

# 1. Interface Mode
Write-Host "1. Interface Mode:" -ForegroundColor White
Write-Host "   [1] Native Desktop GUI (default)" -ForegroundColor Green
Write-Host "   [2] Web Dashboard (Browser)"
$ModeInput = Read-Host "Select mode [1/2, default: 1]"
$IsWeb = ($ModeInput -eq "2" -or $ModeInput -match "^w(eb)?$")

if ($IsWeb) {
    Write-Host "Selected: Web Dashboard Mode`n" -ForegroundColor Green
} else {
    Write-Host "Selected: Native Desktop GUI Mode`n" -ForegroundColor Green
}

# 2. Logs Directory
Write-Host "2. Logs Directory:" -ForegroundColor White
$LogsInput = Read-Host "Enter logs folder [default: ./logs]"
$LogsDir = if ([string]::IsNullOrWhiteSpace($LogsInput)) { "./logs" } else { $LogsInput.Trim() }
Write-Host "Selected logs dir: $LogsDir`n" -ForegroundColor Green

# 3. CSV Output Directory
Write-Host "3. CSV Output Directory:" -ForegroundColor White
$CsvInput = Read-Host "Enter CSV folder [default: ./csv]"
$CsvDir = if ([string]::IsNullOrWhiteSpace($CsvInput)) { "./csv" } else { $CsvInput.Trim() }
Write-Host "Selected CSV dir: $CsvDir`n" -ForegroundColor Green

# 4. Port (if Web mode)
$PortVal = 8080
if ($IsWeb) {
    Write-Host "4. Web Server Port:" -ForegroundColor White
    $PortInput = Read-Host "Enter port number [default: 8080]"
    if (-not [string]::IsNullOrWhiteSpace($PortInput)) {
        $PortVal = [int]$PortInput
    }
    Write-Host "Selected port: $PortVal`n" -ForegroundColor Green
}

# Assemble final args
$FinalArgs = @("--logs", $LogsDir, "--csv", $CsvDir)
if ($IsWeb) {
    $FinalArgs += @("--web", "--port", $PortVal.ToString())
}

Write-Host "------------------------------------------------------" -ForegroundColor Cyan
Write-Host "Launching: $BinPath $($FinalArgs -join ' ')" -ForegroundColor White
Write-Host "------------------------------------------------------`n" -ForegroundColor Cyan

& $BinPath @FinalArgs
exit $LASTEXITCODE

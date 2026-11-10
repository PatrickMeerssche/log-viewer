use crate::csv_loader::{extract_series_data, load_table_data};
use crate::log_scanner::{convert_log_sync, scan_logs_directory, scan_tables_for_log};
use crate::models::ColumnStat;
use serde::Serialize;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Response, Server, StatusCode};
use url::Url;

#[allow(dead_code)]
pub struct WebServerHandle {
    pub port: u16,
    pub stop_flag: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct TableDataResponse {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    stats: Vec<ColumnStat>,
    total_rows: usize,
    page: usize,
    page_size: usize,
}

#[derive(Serialize)]
struct SeriesPointResponse {
    label: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

pub fn start_web_server(
    logs_dir: PathBuf,
    csv_dir: PathBuf,
    script_path: PathBuf,
    preferred_port: u16,
) -> Result<WebServerHandle, String> {
    let mut port = preferred_port;
    let mut server = None;

    for _ in 0..10 {
        match Server::http(format!("127.0.0.1:{}", port)) {
            Ok(s) => {
                server = Some(s);
                break;
            }
            Err(_) => {
                port += 1;
            }
        }
    }

    let server = server.ok_or_else(|| "Failed to bind web server port".to_string())?;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();

    let logs_dir_clone = logs_dir.clone();
    let csv_dir_clone = csv_dir.clone();
    let script_clone = script_path.clone();

    thread::spawn(move || {
        while !stop_clone.load(Ordering::Relaxed) {
            let request = match server.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(Some(req)) => req,
                Ok(None) => continue,
                Err(_) => break,
            };

            let url_str = format!("http://localhost{}", request.url());
            let parsed_url = match Url::parse(&url_str) {
                Ok(u) => u,
                Err(_) => {
                    let resp = Response::empty(StatusCode(400));
                    let _ = request.respond(resp);
                    continue;
                }
            };

            let path = parsed_url.path();
            let queries: std::collections::HashMap<_, _> =
                parsed_url.query_pairs().into_owned().collect();

            let response = match path {
                "/" | "/index.html" => {
                    let mut resp = Response::from_string(INDEX_HTML);
                    resp.add_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                            .unwrap(),
                    );
                    resp
                }
                "/api/logs" => {
                    let logs = scan_logs_directory(&logs_dir_clone, &csv_dir_clone);
                    let json = serde_json::to_string(&logs).unwrap_or_else(|_| "[]".to_string());
                    json_response(json)
                }
                "/api/tables" => {
                    let log_name = queries.get("log").cloned().unwrap_or_default();
                    let log_path = logs_dir_clone.join(&log_name);
                    let safe = crate::log_scanner::safe_stem(
                        log_path.file_stem().unwrap_or_default().to_str().unwrap_or(""),
                    );
                    let output_dir = csv_dir_clone.join(safe);
                    let tables = scan_tables_for_log(&log_path, &output_dir, &csv_dir_clone);
                    let json = serde_json::to_string(&tables).unwrap_or_else(|_| "[]".to_string());
                    json_response(json)
                }
                "/api/convert" => {
                    let log_name = queries.get("log").cloned().unwrap_or_default();
                    let log_path = logs_dir_clone.join(&log_name);
                    let safe = crate::log_scanner::safe_stem(
                        log_path.file_stem().unwrap_or_default().to_str().unwrap_or(""),
                    );
                    let output_dir = csv_dir_clone.join(safe);

                    match convert_log_sync(&log_path, &output_dir, &script_clone) {
                        Ok((count, msg)) => json_response(
                            serde_json::json!({
                                "success": true,
                                "table_count": count,
                                "message": msg
                            })
                            .to_string(),
                        ),
                        Err(err) => json_response(
                            serde_json::json!({
                                "success": false,
                                "error": err
                            })
                            .to_string(),
                        ),
                    }
                }
                "/api/table_data" => {
                    let file_path_str = queries.get("path").cloned().unwrap_or_default();
                    let table_name = queries.get("table").cloned().unwrap_or_default();
                    let page: usize = queries.get("page").and_then(|p| p.parse().ok()).unwrap_or(0);
                    let page_size: usize = queries
                        .get("size")
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(50);
                    let search = queries.get("search").cloned().unwrap_or_default().to_lowercase();

                    let path = PathBuf::from(&file_path_str);
                    match load_table_data(&path, &table_name) {
                        Ok(table) => {
                            let filtered_rows: Vec<Vec<String>> = if search.is_empty() {
                                table.rows
                            } else {
                                table
                                    .rows
                                    .into_iter()
                                    .filter(|r| {
                                        r.iter().any(|cell| cell.to_lowercase().contains(&search))
                                    })
                                    .collect()
                            };

                            let total_rows = filtered_rows.len();
                            let start = (page * page_size).min(total_rows);
                            let end = (start + page_size).min(total_rows);
                            let paged_rows = filtered_rows[start..end].to_vec();

                            let resp_data = TableDataResponse {
                                headers: table.headers,
                                rows: paged_rows,
                                stats: table.stats,
                                total_rows,
                                page,
                                page_size,
                            };
                            let json = serde_json::to_string(&resp_data).unwrap_or_default();
                            json_response(json)
                        }
                        Err(err) => json_response(
                            serde_json::json!({
                                "error": err
                            })
                            .to_string(),
                        ),
                    }
                }
                "/api/series" => {
                    let file_path_str = queries.get("path").cloned().unwrap_or_default();
                    let table_name = queries.get("table").cloned().unwrap_or_default();
                    let x_col = queries.get("x").cloned().unwrap_or_default();
                    let y_cols_str = queries.get("y").cloned().unwrap_or_default();
                    let group_col = queries.get("group").filter(|g| !g.is_empty()).cloned();
                    let downsample: usize = queries
                        .get("limit")
                        .and_then(|l| l.parse().ok())
                        .unwrap_or(10_000);

                    let y_cols: Vec<String> = y_cols_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    let path = PathBuf::from(&file_path_str);
                    match load_table_data(&path, &table_name) {
                        Ok(table) => {
                            let series = extract_series_data(
                                &table,
                                &x_col,
                                &y_cols,
                                group_col.as_deref(),
                                downsample,
                                true,
                                false,
                            );

                            let plotly_series: Vec<SeriesPointResponse> = series
                                .into_iter()
                                .map(|s| {
                                    let mut x = Vec::with_capacity(s.points.len());
                                    let mut y = Vec::with_capacity(s.points.len());
                                    for pt in s.points {
                                        x.push(pt[0]);
                                        y.push(pt[1]);
                                    }
                                    SeriesPointResponse {
                                        label: s.label,
                                        x,
                                        y,
                                    }
                                })
                                .collect();

                            let json = serde_json::to_string(&plotly_series).unwrap_or_default();
                            json_response(json)
                        }
                        Err(err) => json_response(
                            serde_json::json!({
                                "error": err
                            })
                            .to_string(),
                        ),
                    }
                }
                "/api/messages" => {
                    let log_name = queries.get("log").cloned().unwrap_or_default();
                    let log_path = logs_dir_clone.join(&log_name);
                    let safe = crate::log_scanner::safe_stem(
                        log_path.file_stem().unwrap_or_default().to_str().unwrap_or(""),
                    );
                    let output_dir = csv_dir_clone.join(&safe);
                    let messages = crate::messages_loader::load_log_messages(&output_dir, &csv_dir_clone, &safe);

                    #[derive(Serialize)]
                    struct WebMessage {
                        time_rel: String,
                        raw_time: f64,
                        severity: String,
                        source: String,
                        message: String,
                        details: Option<String>,
                    }

                    let out: Vec<WebMessage> = messages
                        .into_iter()
                        .map(|m| WebMessage {
                            time_rel: crate::messages_loader::format_rel_time(m.rel_time_s),
                            raw_time: m.raw_time,
                            severity: m.severity.label().to_string(),
                            source: m.source,
                            message: m.message,
                            details: m.details,
                        })
                        .collect();

                    let json = serde_json::to_string(&out).unwrap_or_else(|_| "[]".to_string());
                    json_response(json)
                }
                _ => Response::from_string("Not Found").with_status_code(StatusCode(404)),
            };

            let _ = request.respond(response);
        }
    });

    Ok(WebServerHandle { port, stop_flag })
}

fn json_response(json: String) -> Response<Cursor<Vec<u8>>> {
    let mut resp = Response::from_string(json);
    resp.add_header(
        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
    );
    resp.add_header(
        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
    );
    resp
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Telemetry & Flight Log Visualizer</title>
<script src="https://cdn.plot.ly/plotly-2.35.2.min.js"></script>
<style>
:root {
  --bg-primary: #12151c;
  --bg-secondary: #1a1e28;
  --bg-card: #222736;
  --bg-hover: #2b3245;
  --text-main: #e2e8f0;
  --text-muted: #94a3b8;
  --accent: #3b82f6;
  --accent-hover: #2563eb;
  --border: #334155;
  --badge-green: #10b981;
  --badge-yellow: #f59e0b;
}
.badge-err { background: #7f1d1d; color: #fecaca; }
.badge-warn { background: #78350f; color: #fde68a; }
.badge-event { background: #1e3a8a; color: #bfdbfe; }
.badge-info { background: #374151; color: #d1d5db; }
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  background-color: var(--bg-primary);
  color: var(--text-main);
  height: 100vh;
  display: flex;
  flex-direction: column;
}
header {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border);
  padding: 12px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}
header h1 { font-size: 1.25rem; font-weight: 700; display: flex; align-items: center; gap: 8px; }
.header-badge { font-size: 0.75rem; background: #1e3a8a; color: #93c5fd; padding: 3px 8px; border-radius: 9999px; }
.container { display: flex; flex: 1; overflow: hidden; }
.sidebar {
  width: 340px;
  background: var(--bg-secondary);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.sidebar-section { padding: 12px; border-bottom: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
.sidebar-title { font-size: 0.8rem; font-weight: 700; text-transform: uppercase; color: var(--text-muted); letter-spacing: 0.05em; display: flex; justify-content: space-between; align-items: center; }
input[type="text"], select {
  background: var(--bg-primary);
  border: 1px solid var(--border);
  color: var(--text-main);
  border-radius: 6px;
  padding: 7px 10px;
  font-size: 0.85rem;
  width: 100%;
}
.btn {
  background: var(--accent);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 6px 12px;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.btn:hover { background: var(--accent-hover); }
.btn-sm { padding: 3px 8px; font-size: 0.75rem; }
.list-container { flex: 1; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 4px; }
.list-item {
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.85rem;
  background: transparent;
  transition: all 0.15s;
}
.list-item:hover { background: var(--bg-hover); }
.list-item.active { background: #1e293b; border-left: 3px solid var(--accent); }
.badge { font-size: 0.7rem; padding: 2px 6px; border-radius: 4px; font-weight: 600; }
.badge-green { background: #064e3b; color: #6ee7b7; }
.badge-yellow { background: #78350f; color: #fde68a; }

.main-content { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.tab-bar {
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border);
  display: flex;
  padding: 0 16px;
}
.tab {
  padding: 12px 18px;
  font-size: 0.9rem;
  font-weight: 600;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  color: var(--text-muted);
}
.tab:hover { color: var(--text-main); }
.tab.active { color: var(--accent); border-bottom-color: var(--accent); }

.tab-pane { flex: 1; display: none; flex-direction: column; overflow: hidden; padding: 16px; }
.tab-pane.active { display: flex; }

.chart-controls {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px;
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  align-items: center;
  margin-bottom: 12px;
}
.control-group { display: flex; flex-direction: column; gap: 4px; min-width: 140px; }
.control-group label { font-size: 0.75rem; color: var(--text-muted); font-weight: 600; }
.chart-wrapper { flex: 1; background: var(--bg-card); border-radius: 8px; border: 1px solid var(--border); position: relative; }
#plotDiv { width: 100%; height: 100%; }

.table-wrapper {
  flex: 1;
  background: var(--bg-card);
  border-radius: 8px;
  border: 1px solid var(--border);
  overflow: auto;
}
table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
th { background: #1a1e28; position: sticky; top: 0; padding: 8px 12px; text-align: left; border-bottom: 2px solid var(--border); color: #cbd5e1; }
td { padding: 6px 12px; border-bottom: 1px solid #283042; font-family: monospace; }
tr:hover td { background: var(--bg-hover); }
.pagination { display: flex; justify-content: space-between; align-items: center; padding-top: 12px; }
</style>
</head>
<body>
<header>
  <h1>Log & Telemetry Visualizer <span class="header-badge">Rust Engine</span></h1>
  <div id="statusHeader" style="font-size: 0.85rem; color: var(--text-muted);">Ready</div>
</header>
<div class="container">
  <div class="sidebar">
    <div class="sidebar-section">
      <div class="sidebar-title">
        <span>Logs in ./logs</span>
        <button class="btn btn-sm" onclick="fetchLogs()">Refresh</button>
      </div>
      <input type="text" id="logFilter" placeholder="Filter logs..." oninput="renderLogList()">
    </div>
    <div class="list-container" id="logList"></div>

    <div class="sidebar-section" style="border-top: 1px solid var(--border);">
      <div class="sidebar-title">
        <span>Tables</span>
        <span id="tableCountBadge" style="font-size: 0.75rem;">0</span>
      </div>
      <input type="text" id="tableFilter" placeholder="Filter tables..." oninput="renderTableList()">
    </div>
    <div class="list-container" id="tableList"></div>
  </div>

  <div class="main-content">
    <div class="tab-bar">
      <div class="tab active" onclick="switchTab('graph')">Graph</div>
      <div class="tab" onclick="switchTab('table')">Table View</div>
      <div class="tab" onclick="switchTab('stats')">Column Statistics</div>
      <div class="tab" id="tab-messages-btn" onclick="switchTab('messages')">Messages & Errors</div>
    </div>

    <!-- Graph Tab -->
    <div class="tab-pane active" id="pane-graph">
      <div class="chart-controls">
        <div class="control-group">
          <label>X Axis</label>
          <select id="selectX" onchange="loadPlot()"></select>
        </div>
        <div class="control-group">
          <label>Y Axis Column(s)</label>
          <select id="selectY" multiple size="3" style="height: 60px;" onchange="loadPlot()"></select>
        </div>
        <div class="control-group">
          <label>Group / Split By</label>
          <select id="selectGroup" onchange="loadPlot()">
            <option value="">(None)</option>
          </select>
        </div>
        <div class="control-group">
          <label>Downsample Limit</label>
          <select id="selectLimit" onchange="loadPlot()">
            <option value="5000">5,000 points</option>
            <option value="10000" selected>10,000 points</option>
            <option value="25000">25,000 points</option>
            <option value="0">All points</option>
          </select>
        </div>
        <button class="btn" style="margin-top: 14px;" onclick="loadPlot()">Update Plot</button>
        <div style="margin-left: auto; display: flex; gap: 6px; align-items: center; margin-top: 14px;">
          <span style="font-size: 0.85rem; color: var(--text-muted); margin-right: 4px;">Save as:</span>
          <button class="btn btn-sm" onclick="exportWebPlot('svg')" title="Save as scalable vector graphic">SVG</button>
          <button class="btn btn-sm" onclick="exportWebPlot('png')" title="Save as high-resolution image">PNG</button>
        </div>
      </div>
      <div class="chart-wrapper">
        <div id="plotDiv"></div>
      </div>
    </div>

    <!-- Table Tab -->
    <div class="tab-pane" id="pane-table">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
        <input type="text" id="tableSearch" placeholder="Search rows..." style="max-width: 300px;" oninput="loadTableData(0)">
        <span id="rowCountLabel" style="font-size: 0.85rem; color: var(--text-muted);">Rows: 0</span>
      </div>
      <div class="table-wrapper">
        <table id="dataTable">
          <thead id="dataThead"></thead>
          <tbody id="dataTbody"></tbody>
        </table>
      </div>
      <div class="pagination">
        <button class="btn btn-sm" onclick="prevPage()">◀ Previous</button>
        <span id="pageLabel" style="font-size: 0.85rem;">Page 1</span>
        <button class="btn btn-sm" onclick="nextPage()">Next ▶</button>
      </div>
    </div>

    <!-- Stats Tab -->
    <div class="tab-pane" id="pane-stats">
      <div class="table-wrapper">
        <table>
          <thead>
            <tr>
              <th>Column Name</th>
              <th>Numeric?</th>
              <th>Total Count</th>
              <th>Nulls</th>
              <th>Min</th>
              <th>Max</th>
              <th>Mean</th>
              <th>Std Dev</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody id="statsTbody"></tbody>
        </table>
      </div>
    </div>

    <!-- Messages Tab -->
    <div class="tab-pane" id="pane-messages">
      <div style="display: flex; gap: 12px; margin-bottom: 12px; align-items: center; flex-wrap: wrap;">
        <input type="text" id="msgSearch" placeholder="Filter messages, errors, subsystems..." style="max-width: 320px;" oninput="renderMessages()">
        <select id="msgSeverityFilter" onchange="renderMessages()" style="max-width: 150px;">
          <option value="ALL">All Severities</option>
          <option value="ERROR">Errors</option>
          <option value="WARN">Warnings</option>
          <option value="EVENT">Events</option>
          <option value="INFO">Info</option>
        </select>
        <span id="msgCountBadge" style="font-size: 0.85rem; color: var(--text-muted);">0 messages</span>
      </div>
      <div class="table-wrapper">
        <table id="msgTable">
          <thead>
            <tr>
              <th style="width: 120px;">Time</th>
              <th style="width: 90px;">Severity</th>
              <th style="width: 100px;">Source</th>
              <th>Message</th>
              <th>Details</th>
            </tr>
          </thead>
          <tbody id="msgTbody"></tbody>
        </table>
      </div>
    </div>
  </div>
</div>

<script>
let logs = [];
let tables = [];
let activeLog = null;
let activeTable = null;
let currentPage = 0;
let totalPages = 1;
let currentStats = [];
let messages = [];

async function fetchLogs() {
  document.getElementById('statusHeader').innerText = 'Scanning logs...';
  const res = await fetch('/api/logs');
  logs = await res.json();
  renderLogList();
  document.getElementById('statusHeader').innerText = `Found ${logs.length} logs in ./logs`;
  if (!activeLog && logs.length > 0) {
    selectLog(logs[0].filename);
  }
}

function renderLogList() {
  const filter = document.getElementById('logFilter').value.toLowerCase();
  const list = document.getElementById('logList');
  list.innerHTML = '';

  logs.filter(l => l.filename.toLowerCase().includes(filter)).forEach(log => {
    const div = document.createElement('div');
    div.className = `list-item ${activeLog === log.filename ? 'active' : ''}`;
    div.onclick = () => selectLog(log.filename);

    let badge = '';
    if (log.status.Converted) {
      badge = `<span class="badge badge-green">${log.status.Converted.table_count} tables</span>`;
    } else if (log.status === 'Converting') {
      badge = `<span class="badge badge-yellow">Converting...</span>`;
    } else {
      badge = `<button class="btn btn-sm" onclick="event.stopPropagation(); convertLog('${log.filename}')">Extract</button>`;
    }

    div.innerHTML = `
      <div style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 200px;">
        <strong>${log.filename}</strong><br>
        <span style="font-size: 0.75rem; color: var(--text-muted);">${log.extension.toUpperCase()} • ${formatSize(log.size_bytes)}</span>
      </div>
      <div>${badge}</div>
    `;
    list.appendChild(div);
  });
}

function formatSize(b) {
  if (b >= 1048576) return (b / 1048576).toFixed(1) + ' MB';
  if (b >= 1024) return (b / 1024).toFixed(0) + ' KB';
  return b + ' B';
}

async function selectLog(filename) {
  activeLog = filename;
  renderLogList();
  document.getElementById('statusHeader').innerText = `Selected log: ${filename}`;
  loadMessages(filename);
  const res = await fetch(`/api/tables?log=${encodeURIComponent(filename)}`);
  tables = await res.json();
  document.getElementById('tableCountBadge').innerText = tables.length;
  renderTableList();
  if (tables.length > 0) {
    selectTable(tables[0]);
  }
}

async function loadMessages(filename) {
  try {
    const res = await fetch(`/api/messages?log=${encodeURIComponent(filename)}`);
    messages = await res.json();
    const errCount = messages.filter(m => m.severity === 'ERROR').length;
    const tabBtn = document.getElementById('tab-messages-btn');
    if (tabBtn) {
      if (errCount > 0) {
        tabBtn.innerText = `Messages & Errors (${errCount})`;
        tabBtn.style.color = '#fca5a5';
      } else if (messages.length > 0) {
        tabBtn.innerText = `Messages (${messages.length})`;
        tabBtn.style.color = '';
      } else {
        tabBtn.innerText = 'Messages & Errors';
        tabBtn.style.color = '';
      }
    }
    renderMessages();
  } catch (e) {
    console.error('Failed to load messages', e);
  }
}

function renderMessages() {
  const search = (document.getElementById('msgSearch')?.value || '').toLowerCase();
  const sevFilter = document.getElementById('msgSeverityFilter')?.value || 'ALL';
  const tbody = document.getElementById('msgTbody');
  if (!tbody) return;

  const filtered = messages.filter(m => {
    if (sevFilter !== 'ALL' && m.severity !== sevFilter) return false;
    if (search && !m.message.toLowerCase().includes(search) && !m.source.toLowerCase().includes(search)) return false;
    return true;
  });

  const badgeEl = document.getElementById('msgCountBadge');
  if (badgeEl) badgeEl.innerText = `${filtered.length} of ${messages.length} messages`;

  tbody.innerHTML = filtered.map(m => {
    let badgeClass = 'badge-info';
    if (m.severity === 'ERROR') badgeClass = 'badge-err';
    else if (m.severity === 'WARN') badgeClass = 'badge-warn';
    else if (m.severity === 'EVENT') badgeClass = 'badge-event';

    return `
      <tr>
        <td style="color: #94a3b8;" title="Raw: ${m.raw_time.toFixed(3)}s">${m.time_rel}</td>
        <td><span class="badge ${badgeClass}">${m.severity}</span></td>
        <td><span class="badge" style="background:#1e293b; color:#94a3b8;">${m.source}</span></td>
        <td style="font-weight: 600; color: ${m.severity === 'ERROR' ? '#fca5a5' : (m.severity === 'WARN' ? '#fde047' : '#e2e8f0')};">${m.message}</td>
        <td style="color: #64748b; font-size: 0.8rem;">${m.details || ''}</td>
      </tr>
    `;
  }).join('');
}

async function convertLog(filename) {
  document.getElementById('statusHeader').innerText = `Extracting tables for ${filename}...`;
  const res = await fetch(`/api/convert?log=${encodeURIComponent(filename)}`, { method: 'POST' });
  const data = await res.json();
  if (data.success) {
    fetchLogs();
    selectLog(filename);
  } else {
    alert("Error converting: " + data.error);
  }
}

function renderTableList() {
  const filter = document.getElementById('tableFilter').value.toLowerCase();
  const list = document.getElementById('tableList');
  list.innerHTML = '';

  tables.filter(t => t.name.toLowerCase().includes(filter)).forEach(table => {
    const div = document.createElement('div');
    div.className = `list-item ${activeTable && activeTable.file_path === table.file_path ? 'active' : ''}`;
    div.onclick = () => selectTable(table);

    div.innerHTML = `
      <div><strong>${table.name}</strong></div>
      <span class="badge badge-green">${table.row_count} rows</span>
    `;
    list.appendChild(div);
  });
}

function selectTable(table) {
  activeTable = table;
  renderTableList();
  document.getElementById('statusHeader').innerText = `Viewing table: ${table.name} (${table.row_count} rows)`;
  loadTableData(0);
}

async function loadTableData(page) {
  if (!activeTable) return;
  currentPage = page;
  const search = document.getElementById('tableSearch').value;
  const url = `/api/table_data?path=${encodeURIComponent(activeTable.file_path)}&table=${encodeURIComponent(activeTable.name)}&page=${page}&size=50&search=${encodeURIComponent(search)}`;
  const res = await fetch(url);
  const data = await res.json();

  currentStats = data.stats || [];
  totalPages = Math.ceil(data.total_rows / 50) || 1;
  document.getElementById('rowCountLabel').innerText = `Rows: ${data.total_rows}`;
  document.getElementById('pageLabel').innerText = `Page ${page + 1} of ${totalPages}`;

  // Populate Selectors
  populateSelectors(data.headers, currentStats);

  // Render Table
  const thead = document.getElementById('dataThead');
  thead.innerHTML = '<tr>' + data.headers.map(h => `<th>${h}</th>`).join('') + '</tr>';

  const tbody = document.getElementById('dataTbody');
  tbody.innerHTML = data.rows.map(r => '<tr>' + r.map(c => `<td>${c}</td>`).join('') + '</tr>').join('');

  // Render Stats
  renderStats(data.stats);

  // Auto load plot if first time
  if (page === 0) {
    loadPlot();
  }
}

function populateSelectors(headers, stats) {
  const selectX = document.getElementById('selectX');
  const selectY = document.getElementById('selectY');
  const selectGroup = document.getElementById('selectGroup');

  const prevX = selectX.value;
  const prevY = Array.from(selectY.selectedOptions).map(o => o.value);
  const prevGroup = selectGroup.value;

  selectX.innerHTML = '<option value="__ROW_INDEX__">Row Index (Sample #)</option>' +
    headers.map(h => `<option value="${h}">${h}</option>`).join('');

  selectY.innerHTML = headers.map(h => `<option value="${h}">${h}</option>`).join('');

  selectGroup.innerHTML = '<option value="">(None)</option>' +
    headers.map(h => `<option value="${h}">${h}</option>`).join('');

  // Auto select best X
  const bestX = headers.find(h => h.toLowerCase() === '_timestamp') ||
                headers.find(h => h.toLowerCase() === 'timeus') || '__ROW_INDEX__';
  selectX.value = prevX || bestX;

  // Auto select best group
  const bestGroup = headers.find(h => h.toLowerCase() === 'inst') ||
                    headers.find(h => h.toLowerCase() === 'id');
  if (bestGroup && !prevGroup) {
    selectGroup.value = bestGroup;
  } else if (prevGroup) {
    selectGroup.value = prevGroup;
  }

  // Auto select best Y
  if (prevY.length > 0) {
    Array.from(selectY.options).forEach(opt => {
      opt.selected = prevY.includes(opt.value);
    });
  } else {
    const interesting = ['curr', 'current_battery', 'volt', 'roll', 'pitch', 'yaw', 'alt'];
    let selectedAny = false;
    for (const intr of interesting) {
      const match = headers.find(h => h.toLowerCase() === intr);
      if (match) {
        Array.from(selectY.options).find(o => o.value === match).selected = true;
        selectedAny = true;
        break;
      }
    }
    if (!selectedAny && headers.length > 1) {
      selectY.options[1].selected = true;
    }
  }
}

function renderStats(stats) {
  const tbody = document.getElementById('statsTbody');
  if (!stats) return;
  tbody.innerHTML = stats.map(s => `
    <tr>
      <td><strong>${s.name}</strong></td>
      <td>${s.is_numeric ? 'Yes' : 'No'}</td>
      <td>${s.total_count}</td>
      <td>${s.null_count}</td>
      <td>${s.min !== null ? s.min.toFixed(3) : '-'}</td>
      <td>${s.max !== null ? s.max.toFixed(3) : '-'}</td>
      <td>${s.mean !== null ? s.mean.toFixed(3) : '-'}</td>
      <td>${s.std_dev !== null ? s.std_dev.toFixed(3) : '-'}</td>
      <td>
        ${s.is_numeric ? `<button class="btn btn-sm" onclick="plotColumn('${s.name}')">Plot</button>` : ''}
      </td>
    </tr>
  `).join('');
}

function plotColumn(colName) {
  const selectY = document.getElementById('selectY');
  Array.from(selectY.options).forEach(opt => {
    opt.selected = (opt.value === colName);
  });
  switchTab('graph');
  loadPlot();
}

async function loadPlot() {
  if (!activeTable) return;
  const xCol = document.getElementById('selectX').value;
  const yCols = Array.from(document.getElementById('selectY').selectedOptions).map(o => o.value);
  const group = document.getElementById('selectGroup').value;
  const limit = document.getElementById('selectLimit').value;

  if (yCols.length === 0) {
    Plotly.purge('plotDiv');
    return;
  }

  const url = `/api/series?path=${encodeURIComponent(activeTable.file_path)}&table=${encodeURIComponent(activeTable.name)}&x=${encodeURIComponent(xCol)}&y=${encodeURIComponent(yCols.join(','))}&group=${encodeURIComponent(group)}&limit=${limit}`;
  const res = await fetch(url);
  const seriesList = await res.json();

  const traces = seriesList.map(s => ({
    name: s.label,
    x: s.x,
    y: s.y,
    type: 'scatter',
    mode: 'lines',
    line: { width: 1.5 }
  }));

  const layout = {
    title: `${activeTable.name}: ${yCols.join(', ')} vs ${xCol}`,
    paper_bgcolor: 'transparent',
    plot_bgcolor: '#1a1e28',
    font: { color: '#e2e8f0' },
    xaxis: { gridcolor: '#334155', title: xCol },
    yaxis: { gridcolor: '#334155', title: yCols.join(', ') },
    hovermode: 'x unified',
    margin: { l: 60, r: 40, t: 60, b: 60 },
    legend: { orientation: 'h', y: 1.15 }
  };

  Plotly.newPlot('plotDiv', traces, layout, { responsive: true });
}

function exportWebPlot(format) {
  const plotDiv = document.getElementById('plotDiv');
  if (!plotDiv || !plotDiv.data || plotDiv.data.length === 0) {
    alert('Please select variables and generate a plot before exporting.');
    return;
  }
  const filename = activeTable ? `${activeTable.name}_telemetry` : 'telemetry_plot';
  Plotly.downloadImage(plotDiv, {
    format: format,
    width: 1920,
    height: 1080,
    filename: filename
  });
}

function prevPage() {
  if (currentPage > 0) loadTableData(currentPage - 1);
}
function nextPage() {
  if (currentPage < totalPages - 1) loadTableData(currentPage + 1);
}

function switchTab(name) {
  document.querySelectorAll('.tab').forEach((t, i) => {
    t.classList.toggle('active', ['graph', 'table', 'stats', 'messages'][i] === name);
  });
  document.querySelectorAll('.tab-pane').forEach((p) => {
    p.classList.toggle('active', p.id === `pane-${name}`);
  });
  if (name === 'graph') {
    setTimeout(() => { Plotly.Plots.resize('plotDiv'); }, 50);
  }
}

fetchLogs();
</script>
</body>
</html>
"#;

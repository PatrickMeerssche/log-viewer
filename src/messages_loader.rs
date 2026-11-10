use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSeverity {
    Error,
    Warning,
    Event,
    Info,
}

impl MessageSeverity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warning => "WARN",
            Self::Event => "EVENT",
            Self::Info => "INFO",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogMessageItem {
    pub raw_time: f64,
    pub rel_time_s: f64,
    pub severity: MessageSeverity,
    pub source: String,
    pub message: String,
    pub details: Option<String>,
}

pub fn load_log_messages(log_output_dir: &Path, csv_base_dir: &Path, safe_stem: &str) -> Vec<LogMessageItem> {
    let mut messages = Vec::new();

    // Possible message files
    let targets = ["MSG", "STATUSTEXT", "ERR", "EV"];

    for target in &targets {
        let mut path_candidate = log_output_dir.join(format!("{}__{}.csv", safe_stem, target));
        if !path_candidate.exists() {
            path_candidate = log_output_dir.join(format!("{}.csv", target));
        }
        if !path_candidate.exists() {
            path_candidate = csv_base_dir.join(format!("{}__{}.csv", safe_stem, target));
        }
        if !path_candidate.exists() {
            path_candidate = PathBuf::from("csv_out").join(format!("{}__{}.csv", safe_stem, target));
        }
        // Fallback: search directory for any file ending with __{target}.csv or {target}.csv
        if !path_candidate.exists() && log_output_dir.exists() && log_output_dir.is_dir() {
            if let Ok(rd) = std::fs::read_dir(log_output_dir) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if fname.ends_with(&format!("__{}.csv", target))
                        || fname.eq_ignore_ascii_case(&format!("{}.csv", target))
                    {
                        path_candidate = p;
                        break;
                    }
                }
            }
        }

        if path_candidate.exists() {
            match *target {
                "MSG" => parse_msg_table(&path_candidate, &mut messages),
                "STATUSTEXT" => parse_statustext_table(&path_candidate, &mut messages),
                "ERR" => parse_err_table(&path_candidate, &mut messages),
                "EV" => parse_ev_table(&path_candidate, &mut messages),
                _ => {}
            }
        }
    }

    if messages.is_empty() {
        return messages;
    }

    // Sort chronologically by raw_time
    messages.sort_by(|a, b| a.raw_time.partial_cmp(&b.raw_time).unwrap_or(std::cmp::Ordering::Equal));

    // Calculate relative time in seconds from first message
    let min_time = messages
        .iter()
        .map(|m| m.raw_time)
        .filter(|t| t.is_finite())
        .fold(f64::INFINITY, f64::min);

    if min_time.is_finite() {
        for msg in &mut messages {
            msg.rel_time_s = msg.raw_time - min_time;
        }
    }

    messages
}

fn parse_msg_table(path: &Path, out: &mut Vec<LogMessageItem>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(file);
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return,
    };

    let (time_idx, is_us) = if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case("_timestamp")) {
        (Some(idx), false)
    } else if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case("timeus")) {
        (Some(idx), true)
    } else {
        (None, false)
    };

    let msg_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("message") || h.eq_ignore_ascii_case("text"));

    let msg_idx = match msg_idx {
        Some(i) => i,
        None => return,
    };

    for record in rdr.records().flatten() {
        let text = match record.get(msg_idx) {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => continue,
        };

        let mut raw_time = time_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if is_us && raw_time > 1000.0 {
            raw_time /= 1e6;
        }

        let severity = classify_message_text(&text);

        out.push(LogMessageItem {
            raw_time,
            rel_time_s: 0.0,
            severity,
            source: "MSG".to_string(),
            message: text,
            details: None,
        });
    }
}

fn parse_statustext_table(path: &Path, out: &mut Vec<LogMessageItem>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(file);
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return,
    };

    let time_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("_timestamp"));
    let text_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("text"));
    let severity_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("severity"));

    let text_idx = match text_idx {
        Some(i) => i,
        None => return,
    };

    for record in rdr.records().flatten() {
        let text = match record.get(text_idx) {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => continue,
        };

        let raw_time = time_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let mav_sev = severity_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<u8>().ok());

        let severity = match mav_sev {
            Some(0..=3) => MessageSeverity::Error,   // MAV_SEVERITY_EMERGENCY, ALERT, CRITICAL, ERROR
            Some(4) => MessageSeverity::Warning,     // MAV_SEVERITY_WARNING
            Some(5..=6) => MessageSeverity::Event,   // MAV_SEVERITY_NOTICE, INFO
            Some(_) => MessageSeverity::Info,        // DEBUG
            None => classify_message_text(&text),
        };

        out.push(LogMessageItem {
            raw_time,
            rel_time_s: 0.0,
            severity,
            source: "STATUSTEXT".to_string(),
            message: text,
            details: mav_sev.map(|s| format!("MAVLink Severity Level: {}", s)),
        });
    }
}

fn parse_err_table(path: &Path, out: &mut Vec<LogMessageItem>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(file);
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return,
    };

    let (time_idx, is_us) = if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case("_timestamp")) {
        (Some(idx), false)
    } else if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case("timeus")) {
        (Some(idx), true)
    } else {
        (None, false)
    };

    let subsys_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("subsys"));
    let ecode_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("ecode"));

    for record in rdr.records().flatten() {
        let mut raw_time = time_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if is_us && raw_time > 1000.0 {
            raw_time /= 1e6;
        }

        let subsys = subsys_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let ecode = ecode_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let subsys_name = decode_subsystem(subsys);
        let is_resolved = ecode == 0;

        let (severity, text) = if is_resolved {
            (
                MessageSeverity::Info,
                format!("{}: Error cleared (ECode=0)", subsys_name),
            )
        } else {
            (
                MessageSeverity::Error,
                format!("{}: FAULT / ERROR detected (ECode={})", subsys_name, ecode),
            )
        };

        out.push(LogMessageItem {
            raw_time,
            rel_time_s: 0.0,
            severity,
            source: "ERR".to_string(),
            message: text,
            details: Some(format!("Subsys ID: {}, ECode: {}", subsys, ecode)),
        });
    }
}

fn parse_ev_table(path: &Path, out: &mut Vec<LogMessageItem>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(file);
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return,
    };

    let (time_idx, is_us) = if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case("_timestamp")) {
        (Some(idx), false)
    } else if let Some(idx) = headers.iter().position(|h| h.eq_ignore_ascii_case("timeus")) {
        (Some(idx), true)
    } else {
        (None, false)
    };

    let id_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("id"));

    for record in rdr.records().flatten() {
        let mut raw_time = time_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if is_us && raw_time > 1000.0 {
            raw_time /= 1e6;
        }

        let id = id_idx
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let (severity, text) = decode_event(id);

        out.push(LogMessageItem {
            raw_time,
            rel_time_s: 0.0,
            severity,
            source: "EV".to_string(),
            message: text,
            details: Some(format!("Event ID: {}", id)),
        });
    }
}

pub fn classify_message_text(text: &str) -> MessageSeverity {
    let lower = text.to_lowercase();

    // Critical Errors / Pre-Arm Failures
    if lower.contains("error")
        || lower.contains("fail")
        || lower.contains("prearm")
        || lower.contains("crash")
        || lower.contains("bad")
        || lower.contains("not detected")
        || lower.contains("inconsistent")
        || lower.contains("denied")
        || lower.contains("lost")
        || lower.contains("timeout")
        || lower.contains("critical")
    {
        MessageSeverity::Error
    }
    // Warnings
    else if lower.contains("warn")
        || lower.contains("caution")
        || lower.contains("fence")
        || lower.contains("running")
        || lower.contains("low")
        || lower.contains("battery")
    {
        MessageSeverity::Warning
    }
    // Flight Events / Mode Changes / Missions
    else if lower.contains("mission")
        || lower.contains("takeoff")
        || lower.contains("land")
        || lower.contains("rtl")
        || lower.contains("arm")
        || lower.contains("disarm")
        || lower.contains("waypoint")
        || lower.contains("reached command")
        || lower.contains("mode")
        || lower.contains("rally")
    {
        MessageSeverity::Event
    }
    // Standard Info / Version / Network
    else {
        MessageSeverity::Info
    }
}

fn decode_subsystem(subsys: u32) -> &'static str {
    match subsys {
        1 => "Main System",
        2 => "Radio Control (RC)",
        3 => "Compass / Magnetometer",
        4 => "Optical Flow",
        5 => "Barometer",
        6 => "Inertial Nav",
        7 => "FailSafe Radio",
        8 => "FailSafe Battery",
        9 => "FailSafe GCS",
        10 => "FailSafe Fence",
        11 => "Flight Mode",
        12 => "GPS",
        13 => "Crash Check",
        14 => "Flip",
        15 => "Autotune",
        16 => "Parachute",
        17 => "EKF Primary Check",
        18 => "Flight Mode Fail",
        19 => "Terrain Database",
        20 => "Navigation System",
        21 => "Thrust Loss Check",
        22 => "FailSafe EKF",
        23 => "Takeoff Check",
        _ => "Subsystem (Unknown)",
    }
}

fn decode_event(id: u32) -> (MessageSeverity, String) {
    match id {
        10 => (MessageSeverity::Event, "Armed".to_string()),
        11 => (MessageSeverity::Event, "Disarmed".to_string()),
        15 => (MessageSeverity::Event, "Auto Armed".to_string()),
        17 => (MessageSeverity::Event, "Land Complete".to_string()),
        18 => (MessageSeverity::Warning, "Lost GPS".to_string()),
        28 => (MessageSeverity::Event, "Takeoff".to_string()),
        49 => (MessageSeverity::Event, "Takeoff Complete".to_string()),
        56 => (MessageSeverity::Warning, "Fence Breached".to_string()),
        57 => (MessageSeverity::Event, "Fence Returned".to_string()),
        62 => (MessageSeverity::Event, "Flight Mode Changed".to_string()),
        _ => (MessageSeverity::Info, format!("Flight Event #{}", id)),
    }
}

pub fn format_rel_time(seconds: f64) -> String {
    if seconds.is_nan() || !seconds.is_finite() {
        return "--:--".to_string();
    }
    let total_s = seconds.max(0.0);
    let hrs = (total_s / 3600.0).floor() as u32;
    let mins = ((total_s % 3600.0) / 60.0).floor() as u32;
    let secs = total_s % 60.0;
    if hrs > 0 {
        format!("+{:02}:{:02}:{:04.1}s", hrs, mins, secs)
    } else {
        format!("+{:02}:{:04.1}s", mins, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_message_text() {
        assert_eq!(classify_message_text("PreArm: RC not found"), MessageSeverity::Error);
        assert_eq!(classify_message_text("PreArm: Gyros inconsistent"), MessageSeverity::Error);
        assert_eq!(classify_message_text("Bad AHRS"), MessageSeverity::Error);
        assert_eq!(classify_message_text("Warning: battery low"), MessageSeverity::Warning);
        assert_eq!(classify_message_text("Mission: 1 Takeoff"), MessageSeverity::Event);
        assert_eq!(classify_message_text("Mode GUIDED"), MessageSeverity::Event);
        assert_eq!(classify_message_text("ArduCopter V4.7.2"), MessageSeverity::Info);
    }

    #[test]
    fn test_format_rel_time() {
        assert_eq!(format_rel_time(0.0), "+00:00.0s");
        assert_eq!(format_rel_time(12.34), "+00:12.3s");
        assert_eq!(format_rel_time(65.5), "+01:05.5s");
        assert_eq!(format_rel_time(3665.5), "+01:01:05.5s");
    }

    #[test]
    fn test_subsystem_and_event_decode() {
        assert_eq!(decode_subsystem(3), "Compass / Magnetometer");
        assert_eq!(decode_subsystem(2), "Radio Control (RC)");

        let (sev, name) = decode_event(10);
        assert_eq!(sev, MessageSeverity::Event);
        assert_eq!(name, "Armed");

        let (sev2, name2) = decode_event(56);
        assert_eq!(sev2, MessageSeverity::Warning);
        assert_eq!(name2, "Fence Breached");
    }
}

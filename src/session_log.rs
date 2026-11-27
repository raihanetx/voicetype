use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// In-memory ring buffer of human-readable session events, so the debug icon
/// can show "everything that happened from open to now" without a file reader.
#[derive(Clone)]
pub struct SessionLog {
    lines: Arc<Mutex<VecDeque<String>>>,
    cap: usize,
}

impl SessionLog {
    pub fn new() -> Self {
        Self { lines: Arc::new(Mutex::new(VecDeque::with_capacity(256))), cap: 256 }
    }

    pub fn push(&self, level: &str, msg: impl AsRef<str>) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!("[{}] {:5} {}", short_time(ts), level, msg.as_ref());
        // Also go to the persistent file log.
        match level {
            "ERROR" => tracing::error!("{}", msg.as_ref()),
            "WARN" => tracing::warn!("{}", msg.as_ref()),
            _ => tracing::info!("{}", msg.as_ref()),
        }
        let mut g = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        if g.len() == self.cap {
            g.pop_front();
        }
        g.push_back(line);
    }

    pub fn info(&self, msg: impl AsRef<str>) { self.push("INFO", msg); }
    pub fn warn(&self, msg: impl AsRef<str>) { self.push("WARN", msg); }
    pub fn error(&self, msg: impl AsRef<str>) { self.push("ERROR", msg); }

    /// Full report as one string, newest last, for display/copy.
    pub fn report(&self) -> String {
        let g = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_empty() {
            return "(no events yet)".to_string();
        }
        g.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    pub fn len(&self) -> usize {
        self.lines.lock().map(|g| g.len()).unwrap_or(0)
    }
}

/// HH:MM:SS from a unix timestamp (UTC). Good enough for a debug log.
fn short_time(secs: u64) -> String {
    let s = secs % 86400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

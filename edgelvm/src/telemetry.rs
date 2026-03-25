use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub fn record_log(message: impl Into<String>) {
    let entry = format!("[{}] {}", unix_timestamp(), message.into());
    let logs = LOGS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut logs) = logs.lock() {
        logs.push(entry);
        let overflow = logs.len().saturating_sub(200);
        if overflow > 0 {
            logs.drain(0..overflow);
        }
    }
}

pub fn recent_logs() -> Vec<String> {
    LOGS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .map(|logs| logs.clone())
        .unwrap_or_default()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

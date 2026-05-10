use std::sync::Mutex;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct GhCall {
    pub timestamp: DateTime<Utc>,
    pub command: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

static GH_CALL_LOG: Mutex<Option<VecDeque<GhCall>>> = Mutex::new(None);

pub fn init_logging(log_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(log_dir)?;
    let log_file = log_dir.join("ghnotify.log");
    
    let file_appender = std::fs::File::create(log_file)?;
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(file_appender))
        .init();

    let mut log = GH_CALL_LOG.lock().unwrap();
    *log = Some(VecDeque::with_capacity(100));

    Ok(())
}

pub fn record_gh_call(command: String, exit_code: i32, duration_ms: u64) {
    let mut log = GH_CALL_LOG.lock().unwrap();
    if let Some(ref mut deque) = *log {
        if deque.len() >= 100 {
            deque.pop_front();
        }
        deque.push_back(GhCall {
            timestamp: Utc::now(),
            command,
            exit_code,
            duration_ms,
        });
    }
}

pub fn get_gh_calls() -> Vec<GhCall> {
    let log = GH_CALL_LOG.lock().unwrap();
    log.as_ref()
        .map(|deque| deque.iter().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gh_call_log() {
        // Reset log for test
        {
            let mut log = GH_CALL_LOG.lock().unwrap();
            *log = Some(VecDeque::with_capacity(100));
        }

        record_gh_call("gh api user".to_string(), 0, 150);
        let calls = get_gh_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].command, "gh api user");
        assert_eq!(calls[0].exit_code, 0);
        assert_eq!(calls[0].duration_ms, 150);
    }
}

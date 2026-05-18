use crate::config::AppConfig;
use crate::ui::events::AppEvent;
use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use tokio::sync::mpsc;

#[allow(missing_debug_implementations)]
pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new(config_path: &Path, tx: mpsc::Sender<AppEvent>) -> Result<Self> {
        let config_path = config_path.to_path_buf();
        let config_path_clone = config_path.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res
                && matches!(event.kind, EventKind::Modify(_))
                && event_targets_path(&event, &config_path_clone)
                && let Ok(content) = fs::read_to_string(&config_path_clone)
                && let Ok(config) = toml::from_str::<AppConfig>(&content)
            {
                let _ = tx.blocking_send(AppEvent::ConfigReloaded(config));
            }
        })?;

        watcher.watch(config_path.parent().unwrap(), RecursiveMode::NonRecursive)?;

        Ok(Self { _watcher: watcher })
    }
}

fn event_targets_path(event: &Event, target: &Path) -> bool {
    let target_name = target.file_name();
    event.paths.iter().any(|p| p == target || p.file_name() == target_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_config_watcher() -> Result<()> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_micros();
        let temp_dir = std::env::temp_dir().join(format!("ghwatch_test_config_{now}"));
        fs::create_dir_all(&temp_dir)?;
        let config_path = temp_dir.join("config.toml");

        let mut f = fs::File::create(&config_path)?;
        f.write_all(b"[queries]\n")?;
        f.sync_all()?;

        let (tx, mut rx) = mpsc::channel(1);
        let _watcher = ConfigWatcher::new(&config_path, tx)?;

        let mut f = fs::OpenOptions::new().write(true).truncate(true).open(&config_path)?;
        f.write_all(
            br#"
polling_interval_ms = 5000
current_user = "testuser"
unfollow_timeout_mins = 30
[[queries]]
name = "Test"
search = "is:pr"
interval = "1m"
enabled = true
"#,
        )?;
        f.sync_all()?;

        for _ in 0..10 {
            if let Ok(event) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await
                && matches!(event, Some(AppEvent::ConfigReloaded(_)))
            {
                break;
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);

        Ok(())
    }
}

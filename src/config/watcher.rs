use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::Path;
use tokio::sync::mpsc;
use crate::ui::events::AppEvent;
use crate::config::AppConfig;
use std::fs;

pub struct ConfigWatcher {
    watcher: notify::RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new(config_path: &Path, tx: mpsc::Sender<AppEvent>) -> Result<Self> {
        let config_path = config_path.to_path_buf();
        let config_path_clone = config_path.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_)) {
                    // Try to reload config
                    if let Ok(content) = fs::read_to_string(&config_path_clone) {
                        if let Ok(config) = toml::from_str::<AppConfig>(&content) {
                            let _ = tx.blocking_send(AppEvent::ConfigReloaded(config));
                        }
                    }
                }
            }
        })?;

        watcher.watch(config_path.parent().unwrap(), RecursiveMode::NonRecursive)?;

        Ok(Self { watcher })
    }
}

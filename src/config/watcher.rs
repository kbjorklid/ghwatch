use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::Path;
use tokio::sync::mpsc;
use crate::ui::events::AppEvent;
use crate::config::AppConfig;
use crate::domain::pr::PullRequest;
use std::fs;

pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new(config_path: &Path, tx: mpsc::Sender<AppEvent>) -> Result<Self> {
        let config_path = config_path.to_path_buf();
        let config_path_clone = config_path.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res && matches!(event.kind, EventKind::Modify(_)) {
                // Try to reload config
                if let Ok(content) = fs::read_to_string(&config_path_clone) 
                    && let Ok(config) = toml::from_str::<AppConfig>(&content) {
                    let _ = tx.blocking_send(AppEvent::ConfigReloaded(config));
                }
            }
        })?;

        watcher.watch(config_path.parent().unwrap(), RecursiveMode::NonRecursive)?;

        Ok(Self { _watcher: watcher })
    }
}

pub struct StateWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl StateWatcher {
    pub fn new(state_path: &Path, tx: mpsc::Sender<AppEvent>) -> Result<Self> {
        let state_path = state_path.to_path_buf();
        let state_path_clone = state_path.clone();
        
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res && matches!(event.kind, EventKind::Modify(_)) {
                // Try to reload state
                if let Ok(content) = fs::read_to_string(&state_path_clone) {
                    // This is a bit hacky because we don't want to import storage::local here
                    // But we can just use a simple deserialization since we know the format
                    #[derive(serde::Deserialize)]
                    struct StateFile { prs: Vec<PullRequest> }
                    if let Ok(state) = toml::from_str::<StateFile>(&content) {
                        let _ = tx.blocking_send(AppEvent::StateReloaded(state.prs));
                    }
                }
            }
        })?;

        watcher.watch(state_path.parent().unwrap(), RecursiveMode::NonRecursive)?;

        Ok(Self { _watcher: watcher })
    }
}

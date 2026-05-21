use crate::domain::attention::AttentionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum GroupMode {
    #[default]
    None,
    Repo,
    Author,
    Status,
    MyVsOther,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Column {
    Author,
    Age,
    Diff,
    Comments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub queries: Vec<QueryConfig>,
    pub polling_interval_ms: u64,
    #[serde(default = "default_true")]
    pub use_nerd_fonts: bool,
    pub current_user: String,
    pub unfollow_timeout_mins: u64,
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    #[serde(default)]
    pub group_by: GroupMode,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_columns")]
    pub visible_columns: Vec<Column>,
    #[serde(default)]
    pub attention: AttentionConfig,
    #[serde(default)]
    pub max_age_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    pub name: String,
    pub search: String,
    pub interval: String,
    pub enabled: bool,
}

const fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_columns() -> Vec<Column> {
    vec![Column::Author, Column::Age, Column::Diff, Column::Comments]
}

impl AppConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            queries: vec![QueryConfig {
                name: "My PRs".to_string(),
                search: "is:pr author:@me state:open".to_string(),
                interval: "60s".to_string(),
                enabled: true,
            }],
            polling_interval_ms: 30000,
            use_nerd_fonts: true,
            current_user: String::new(),
            unfollow_timeout_mins: 60,
            show_status_bar: true,
            group_by: GroupMode::None,
            theme: "dark".to_string(),
            visible_columns: default_columns(),
            attention: AttentionConfig::default(),
            max_age_days: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_attention_roundtrip_json() {
        let mut config = AppConfig::default();
        config.attention.open_in_browser_marks_seen = true;
        let json = serde_json::to_string(&config).expect("json serialization failed");
        let loaded: AppConfig = serde_json::from_str(&json).expect("json deserialization failed");
        assert!(loaded.attention.open_in_browser_marks_seen);
    }

    #[test]
    fn test_serialize_attention_roundtrip() {
        let mut config = AppConfig::default();
        config.attention.open_in_browser_marks_seen = true;
        let serialized = toml::to_string(&config).expect("serialization failed");
        assert!(
            serialized.contains("open_in_browser_marks_seen = true"),
            "attention field missing from serialized output:\n{serialized}"
        );
        let loaded: AppConfig = toml::from_str(&serialized).expect("deserialization failed");
        assert!(loaded.attention.open_in_browser_marks_seen);
    }
}

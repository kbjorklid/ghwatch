use serde::{Deserialize, Serialize};

pub mod watcher;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub queries: Vec<QueryConfig>,
    pub polling_interval_ms: u64,
    #[serde(default = "default_true")]
    pub use_nerd_fonts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    pub name: String,
    pub search: String,
    pub interval: String,
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            queries: vec![
                QueryConfig {
                    name: "My PRs".to_string(),
                    search: "is:pr author:@me state:open".to_string(),
                    interval: "60s".to_string(),
                    enabled: true,
                }
            ],
            polling_interval_ms: 30000,
            use_nerd_fonts: true,
        }
    }
}

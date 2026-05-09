use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};
use crate::ui::events::AppEvent;
use crate::domain::ports::GithubProvider;
use crate::config::AppConfig;

pub struct PollingWorker {
    config: AppConfig,
    github: Arc<dyn GithubProvider>,
    event_tx: mpsc::Sender<AppEvent>,
    last_polled: Vec<Option<Instant>>,
}

impl PollingWorker {
    pub fn new(config: AppConfig, github: Arc<dyn GithubProvider>, event_tx: mpsc::Sender<AppEvent>) -> Self {
        let num_queries = config.queries.len();
        Self {
            config,
            github,
            event_tx,
            last_polled: vec![None; num_queries],
        }
    }

    pub async fn start(mut self) {
        let mut interval = time::interval(Duration::from_millis(self.config.polling_interval_ms));
        let mut query_index = 0;

        loop {
            interval.tick().await;

            if self.config.queries.is_empty() {
                continue;
            }

            // Rate limit check once per cycle or occasionally
            if query_index == 0
                && let Ok(rate) = self.github.fetch_rate_limit().await {
                    if rate.remaining < 50 {
                        let _ = self.event_tx.send(AppEvent::Error("Rate limit critical! Pausing polling...".to_string())).await;
                        time::sleep(Duration::from_secs(300)).await; // Long wait
                        continue;
                    } else if rate.remaining < 100 {
                        let _ = self.event_tx.send(AppEvent::Error("Rate limit low. Slowing down...".to_string())).await;
                        time::sleep(Duration::from_secs(60)).await;
                    }
                }

            // Round-robin
            let query = &self.config.queries[query_index];
            let now = Instant::now();
            
            let should_poll = if let Some(last) = self.last_polled[query_index] {
                let interval_dur = parse_duration(&query.interval).unwrap_or(Duration::from_secs(60));
                now.duration_since(last) >= interval_dur
            } else {
                true
            };

            if query.enabled && should_poll {
                match self.github.fetch_prs_by_query(&query.search).await {
                    Ok(prs) => {
                        self.last_polled[query_index] = Some(now);
                        let _ = self.event_tx.send(AppEvent::PrsUpdated {
                            query_name: query.name.clone(),
                            prs,
                        }).await;
                    }
                    Err(e) => {
                        let _ = self.event_tx.send(AppEvent::Error(format!("Polling error ({}): {}", query.name, e))).await;
                    }
                }
            }

            query_index = (query_index + 1) % self.config.queries.len();
        }
    }
}

fn parse_duration(s: &str) -> Option<Duration> {
    if let Some(stripped) = s.strip_suffix('s') {
        stripped.parse::<u64>().ok().map(Duration::from_secs)
    } else if let Some(stripped) = s.strip_suffix('m') {
        stripped.parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
    } else {
        s.parse::<u64>().ok().map(Duration::from_secs)
    }
}

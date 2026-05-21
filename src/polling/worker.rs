use crate::config::AppConfig;
use crate::domain::ports::{GithubProvider, StateRepository};
use crate::ui::events::AppEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};

#[allow(missing_debug_implementations)]
pub struct PollingWorker {
    config: AppConfig,
    github: Arc<dyn GithubProvider>,
    state_repo: Arc<dyn StateRepository>,
    event_tx: mpsc::Sender<AppEvent>,
    last_polled: Vec<Option<Instant>>,
}

impl PollingWorker {
    pub fn new(
        config: AppConfig,
        github: Arc<dyn GithubProvider>,
        state_repo: Arc<dyn StateRepository>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        let num_queries = config.queries.len();
        Self { config, github, state_repo, event_tx, last_polled: vec![None; num_queries] }
    }

    pub async fn start(mut self) {
        let mut interval = time::interval(Duration::from_millis(self.config.polling_interval_ms));
        let mut query_index = 0;
        let mut first_cycle_complete = false;

        loop {
            interval.tick().await;

            if self.config.queries.is_empty() {
                continue;
            }

            if query_index == 0 {
                let _ = self.event_tx.send(AppEvent::PollCycleStarted).await;

                // Lease interval: slightly less than tick to account for processing time.
                let lease_interval =
                    Duration::from_millis(self.config.polling_interval_ms.saturating_sub(200));
                let is_lease_holder =
                    self.state_repo.try_acquire_poll_lease(lease_interval).unwrap_or(true);

                if !is_lease_holder {
                    // Another instance is polling; refresh UI from DB.
                    if let Ok(prs) = self.state_repo.load_state() {
                        let _ = self
                            .event_tx
                            .send(AppEvent::PrsUpdated { query_name: "db-reload".to_string(), prs })
                            .await;
                    }
                    query_index = (query_index + 1) % self.config.queries.len();
                    if !first_cycle_complete {
                        first_cycle_complete = true;
                        let _ = self.event_tx.send(AppEvent::InitialSyncDone).await;
                    }
                    continue;
                }

                // Rate limit check once per cycle
                if let Ok(rate) = self.github.fetch_rate_limit().await {
                    if rate.remaining < 50 {
                        let _ = self
                            .event_tx
                            .send(AppEvent::Error(
                                "Rate limit critical! Pausing polling...".to_string(),
                            ))
                            .await;
                        time::sleep(Duration::from_mins(5)).await;
                        continue;
                    } else if rate.remaining < 100 {
                        let _ = self
                            .event_tx
                            .send(AppEvent::Error("Rate limit low. Slowing down...".to_string()))
                            .await;
                        time::sleep(Duration::from_mins(1)).await;
                    }
                }
            }

            // Round-robin
            let query = &self.config.queries[query_index];
            let now = Instant::now();

            let should_poll = if let Some(last) = self.last_polled[query_index] {
                let interval_dur =
                    parse_duration(&query.interval).unwrap_or(Duration::from_mins(1));
                now.duration_since(last) >= interval_dur
            } else {
                true
            };

            if query.enabled && should_poll {
                let effective_query =
                    apply_age_cutoff(&query.search, self.config.max_age_days, chrono::Utc::now());
                match self.github.fetch_prs_by_query(&effective_query, None).await {
                    Ok(prs) => {
                        self.last_polled[query_index] = Some(now);
                        let _ = self
                            .event_tx
                            .send(AppEvent::PrsUpdated { query_name: query.name.clone(), prs })
                            .await;
                    }
                    Err(e) => {
                        let _ = self
                            .event_tx
                            .send(AppEvent::Error(format!("Polling error ({}): {}", query.name, e)))
                            .await;
                    }
                }
            }

            query_index = (query_index + 1) % self.config.queries.len();
            if query_index == 0 && !first_cycle_complete {
                first_cycle_complete = true;
                let _ = self.event_tx.send(AppEvent::InitialSyncDone).await;
            }
        }
    }
}

pub(crate) fn apply_age_cutoff(
    query: &str,
    max_age_days: Option<u32>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    match max_age_days {
        None | Some(0) => query.to_string(),
        Some(days) => {
            let cutoff = now - chrono::Duration::days(i64::from(days));
            format!("{query} updated:>={}", cutoff.format("%Y-%m-%d"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::QueryConfig;
    use crate::domain::attention::AttentionState;
    use crate::domain::pr::{
        CIStatus, MergeableStatus, PRStatus, PullRequest, RateLimitStatus, ReviewStatus,
    };
    use async_trait::async_trait;
    use mockall::mock;

    mock! {
        pub GithubProvider {}
        #[async_trait]
        impl GithubProvider for GithubProvider {
            async fn fetch_prs_by_query(&self, query: &str, limit: Option<u32>) -> anyhow::Result<Vec<PullRequest>>;
            async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> anyhow::Result<PullRequest>;
            async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> anyhow::Result<Vec<CheckRun>>;
            async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> anyhow::Result<Vec<TimelineEvent>>;
            async fn fetch_rate_limit(&self) -> anyhow::Result<RateLimitStatus>;
            async fn fetch_current_user(&self) -> anyhow::Result<String>;
            async fn open_pr_in_browser(&self, url: &str) -> anyhow::Result<()>;
        }
    }

    use crate::domain::pr::{CheckRun, TimelineEvent};

    mock! {
        pub StateRepository {}
        impl StateRepository for StateRepository {
            fn save_state(&self, prs: &[PullRequest]) -> anyhow::Result<()>;
            fn load_state(&self) -> anyhow::Result<Vec<PullRequest>>;
            fn save_archive(&self, prs: &[PullRequest]) -> anyhow::Result<()>;
            fn load_archive(&self) -> anyhow::Result<Vec<PullRequest>>;
            fn archive_pr(&self, pr: PullRequest) -> anyhow::Result<()>;
            fn try_acquire_poll_lease(&self, interval: std::time::Duration) -> anyhow::Result<bool>;
            fn load_config_json(&self) -> anyhow::Result<Option<String>>;
            fn save_config_json(&self, json: &str) -> anyhow::Result<()>;
        }
    }

    #[tokio::test]
    async fn test_polling_worker_cycle() {
        let mut github = MockGithubProvider::new();
        let mut state_repo = MockStateRepository::new();
        let (tx, mut rx) = mpsc::channel(10);

        let config = AppConfig {
            queries: vec![QueryConfig {
                name: "test".to_string(),
                search: "search".to_string(),
                interval: "1s".to_string(),
                enabled: true,
            }],
            polling_interval_ms: 10,
            ..Default::default()
        };

        state_repo.expect_try_acquire_poll_lease().returning(|_| Ok(true));

        github
            .expect_fetch_rate_limit()
            .returning(|| Ok(RateLimitStatus { limit: 5000, remaining: 4000, reset_at: 0 }));

        github.expect_fetch_prs_by_query().returning(|_, _| {
            Ok(vec![PullRequest {
                id: "1".to_string(),
                number: 1,
                title: "Test".to_string(),
                author: "alice".to_string(),
                repo: "org/repo".to_string(),
                status: PRStatus::Open,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
                additions: 0,
                deletions: 0,
                review_status: ReviewStatus::Pending,
                comment_count: 0,
                unresolved_count: 0,
                total_resolvable_count: 0,
                conversational_count: 0,
                ci_status: CIStatus::Passing,
                mergeable: MergeableStatus::Unknown,
                head_ref: String::new(),
                body: String::new(),
                url: String::new(),
                requested_reviewers: vec![],
                reviewers: vec![],
                is_draft: false,
                matched_queries: Vec::new(),
                last_seen_at: None,
                last_seen_unresolved_count: 0,
                last_seen_total_resolvable_count: 0,
                last_seen_conversational_count: 0,
                attention_state: AttentionState::default(),
            }])
        });

        let worker = PollingWorker::new(config, Arc::new(github), Arc::new(state_repo), tx);

        let handle = tokio::spawn(worker.start());

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(event1, AppEvent::PollCycleStarted));

        let event2 = rx.recv().await.unwrap();
        if let AppEvent::PrsUpdated { query_name, prs } = event2 {
            assert_eq!(query_name, "test");
            assert_eq!(prs.len(), 1);
        } else {
            panic!("Unexpected event: {event2:?}");
        }

        handle.abort();
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn test_apply_age_cutoff_none_returns_query_unchanged() {
        let out = apply_age_cutoff("is:pr author:@me", None, fixed_now());
        assert_eq!(out, "is:pr author:@me");
    }

    #[test]
    fn test_apply_age_cutoff_zero_returns_query_unchanged() {
        let out = apply_age_cutoff("is:pr author:@me", Some(0), fixed_now());
        assert_eq!(out, "is:pr author:@me");
    }

    #[test]
    fn test_apply_age_cutoff_appends_updated_qualifier() {
        let out = apply_age_cutoff("is:pr author:@me", Some(14), fixed_now());
        assert_eq!(out, "is:pr author:@me updated:>=2026-05-04");
    }

    #[test]
    fn test_apply_age_cutoff_leaves_existing_updated_qualifier() {
        let out = apply_age_cutoff("is:pr author:@me updated:>=2020-01-01", Some(7), fixed_now());
        assert_eq!(out, "is:pr author:@me updated:>=2020-01-01 updated:>=2026-05-11");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("60s"), Some(Duration::from_mins(1)));
        assert_eq!(parse_duration("5m"), Some(Duration::from_mins(5)));
        assert_eq!(parse_duration("10"), Some(Duration::from_secs(10)));
        assert_eq!(parse_duration("invalid"), None);
    }
}

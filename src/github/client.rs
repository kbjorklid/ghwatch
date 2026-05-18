use crate::domain::ports::GithubProvider;
use crate::domain::pr::{CheckRun, PullRequest, TimelineEvent};
use crate::github::models::{RawCheckRun, RawPullRequest, RawTimelineEvent};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json;
use tokio::process::Command;

use crate::github::rate_limit::RateLimitTracker;

pub(crate) fn map_timeline_event(r: RawTimelineEvent) -> TimelineEvent {
    let event_type = match r.typename.as_str() {
        "commented" => "IssueComment",
        "reviewed" => "PullRequestReview",
        "committed" => "Commit",
        "merged" => "MergedEvent",
        "closed" => "ClosedEvent",
        "reopened" => "ReopenedEvent",
        "labeled" => "LabeledEvent",
        "unlabeled" => "UnlabeledEvent",
        other => other,
    };

    let content = match event_type {
        "IssueComment" => r.body.clone(),
        "PullRequestReview" => {
            let state = r.state.clone().unwrap_or_else(|| "COMMENTED".to_string()).to_uppercase();
            let body = r.body.as_ref().map(|b| format!(": {b}")).unwrap_or_default();
            Some(format!("{state}{body}"))
        }
        "MergedEvent" => Some("merged this pull request".to_string()),
        "ClosedEvent" => Some("closed this pull request".to_string()),
        "ReopenedEvent" => Some("reopened this pull request".to_string()),
        "LabeledEvent" => r.label.as_ref().map(|l| format!("added label: {}", l.name)),
        "UnlabeledEvent" => r.label.as_ref().map(|l| format!("removed label: {}", l.name)),
        "Commit" => r.message.clone(),
        _ => None,
    };

    TimelineEvent {
        id: r.node_id.unwrap_or_default(),
        event_type: event_type.to_string(),
        actor: r.actor.map_or_else(|| "unknown".to_string(), |a| a.login),
        created_at: r.created_at.unwrap_or_default(),
        content,
        reviewer_login: None,
    }
}

#[derive(Debug)]
pub struct GhCliClient {
    pub rate_limit: RateLimitTracker,
}

impl Default for GhCliClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GhCliClient {
    #[must_use]
    pub const fn new() -> Self {
        Self { rate_limit: RateLimitTracker::new() }
    }

    async fn run_gh(&self, args: &[&str]) -> Result<String> {
        let start = std::time::Instant::now();
        let output =
            Command::new("gh").args(args).output().await.context("Failed to execute gh CLI")?;

        let duration = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let command_str = format!("gh {}", args.join(" "));
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let log_output = if output.status.success() { stdout.clone() } else { stderr.clone() };

        crate::logging::record_gh_call(command_str, exit_code, duration, log_output);

        if !output.status.success() {
            return Err(anyhow::anyhow!("gh CLI error: {stderr}"));
        }

        Ok(stdout)
    }
}

#[async_trait]
impl GithubProvider for GhCliClient {
    async fn fetch_prs_by_query(
        &self,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PullRequest>> {
        let gql_limit = limit.unwrap_or(100);
        let gql_query = format!(
            r"
            query($q: String!) {{
                search(query: $q, type: ISSUE, first: {gql_limit}) {{
                    nodes {{
                        ... on PullRequest {{
                            id
                            number
                            title
                            author {{ login }}
                            repository {{ nameWithOwner }}
                            state
                            isDraft
                            createdAt
                            updatedAt
                            body
                            url
                            additions
                            deletions
                            reviewDecision
                            mergeable
                            mergeStateStatus
                            headRefOid
                            comments {{ totalCount }}
                            reviewThreads(first: 100) {{
                                nodes {{
                                    isResolved
                                }}
                            }}
                            statusCheckRollup: statusCheckRollup {{
                                state
                            }}
                            reviewRequests(first: 10) {{
                                nodes {{
                                    requestedReviewer {{
                                        __typename
                                        ... on User {{ login }}
                                        ... on Team {{ name }}
                                    }}
                                }}
                            }}
                            latestReviews(first: 10) {{
                                nodes {{
                                    author {{ login }}
                                    state
                                }}
                            }}
                        }}
                    }}
                }}
            }}
        "
        );

        let output = self
            .run_gh(&[
                "api",
                "graphql",
                "-f",
                &format!("q={query}"),
                "-f",
                &format!("query={gql_query}"),
            ])
            .await?;

        let val: serde_json::Value =
            serde_json::from_str(&output).context("Failed to parse GraphQL response")?;

        if let Some(errors) = val.get("errors") {
            return Err(anyhow::anyhow!("GraphQL error: {errors}"));
        }

        let nodes = val
            .get("data")
            .and_then(|d| d.get("search"))
            .and_then(|s| s.get("nodes"))
            .and_then(|n| n.as_array())
            .ok_or_else(|| anyhow::anyhow!("Unexpected GraphQL response structure"))?;

        let mut prs = Vec::new();
        for node in nodes {
            if node.is_null() {
                continue;
            }

            // Map GraphQL structure to RawPullRequest structure for compatibility
            let mut raw_val = node.clone();

            // Extract counts
            let comments_count = node
                .get("comments")
                .and_then(|c| c.get("totalCount"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let review_threads_nodes =
                node.get("reviewThreads").and_then(|r| r.get("nodes")).and_then(|n| n.as_array());

            let total_resolvable = review_threads_nodes.map_or(0, |n| n.len() as u64);
            let unresolved = review_threads_nodes.map_or(0, |nodes| {
                nodes
                    .iter()
                    .filter(|n| {
                        n.get("isResolved").and_then(serde_json::Value::as_bool) == Some(false)
                    })
                    .count() as u64
            });

            if let Some(obj) = raw_val.as_object_mut() {
                obj.insert("commentsCount".to_string(), serde_json::Value::from(comments_count));
                obj.insert(
                    "review_comments".to_string(),
                    serde_json::Value::from(total_resolvable),
                );
                obj.insert("unresolved_count".to_string(), serde_json::Value::from(unresolved));
                obj.insert(
                    "total_resolvable_count".to_string(),
                    serde_json::Value::from(total_resolvable),
                );

                // Remove GraphQL specific objects that conflict with RawPullRequest types
                obj.remove("comments");
                obj.remove("reviewThreads");

                // Author mapping
                if let Some(author_obj) = node.get("author").and_then(|a| a.as_object()) {
                    obj.insert("author".to_string(), serde_json::Value::from(author_obj.clone()));
                }

                // Repository mapping
                if let Some(repo_obj) = node.get("repository").and_then(|r| r.as_object()) {
                    obj.insert("repository".to_string(), serde_json::Value::from(repo_obj.clone()));
                }

                // Flatten Connections (reviewRequests, latestReviews)
                for field in &["reviewRequests", "latestReviews"] {
                    if let Some(nodes) = node.get(*field).and_then(|f| f.get("nodes")) {
                        obj.insert(field.to_string(), nodes.clone());
                    }
                }
            }

            let raw: RawPullRequest = serde_json::from_value(raw_val.clone()).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to map GraphQL node to RawPullRequest: {e}. Node: {raw_val}"
                )
            })?;
            prs.push(raw.into());
        }

        Ok(prs)
    }

    async fn fetch_pr_details(&self, repo: &str, pr_number: u32) -> Result<PullRequest> {
        let (owner, name) =
            repo.split_once('/').ok_or_else(|| anyhow::anyhow!("Invalid repo format: {repo}"))?;

        let gql_query = format!(
            r#"
            query {{
                repository(owner: "{owner}", name: "{name}") {{
                    pullRequest(number: {pr_number}) {{
                        id
                        number
                        title
                        author {{ login }}
                        repository {{ nameWithOwner }}
                        state
                        isDraft
                        createdAt
                        updatedAt
                        body
                        url
                        additions
                        deletions
                        reviewDecision
                        mergeable
                        mergeStateStatus
                        headRefOid
                        comments {{ totalCount }}
                        reviewThreads(first: 100) {{
                            nodes {{
                                isResolved
                            }}
                        }}
                        statusCheckRollup {{
                            state
                        }}
                        reviewRequests(first: 10) {{
                            nodes {{
                                requestedReviewer {{
                                    ... on User {{ login }}
                                    ... on Team {{ name }}
                                }}
                            }}
                        }}
                        latestReviews(first: 10) {{
                            nodes {{
                                author {{ login }}
                                state
                            }}
                        }}
                    }}
                }}
            }}
        "#
        );

        let output = self.run_gh(&["api", "graphql", "-f", &format!("query={gql_query}")]).await?;

        let val: serde_json::Value =
            serde_json::from_str(&output).context("Failed to parse GraphQL response for detail")?;

        if let Some(errors) = val.get("errors") {
            return Err(anyhow::anyhow!("GraphQL error: {errors}"));
        }

        let node = val
            .get("data")
            .and_then(|d| d.get("repository"))
            .and_then(|r| r.get("pullRequest"))
            .ok_or_else(|| anyhow::anyhow!("PR not found or unexpected response"))?;

        let mut raw_val = node.clone();

        // Extract counts
        let comments_count = node
            .get("comments")
            .and_then(|c| c.get("totalCount"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let review_threads_nodes =
            node.get("reviewThreads").and_then(|r| r.get("nodes")).and_then(|n| n.as_array());

        let total_resolvable = review_threads_nodes.map_or(0, |n| n.len() as u64);
        let unresolved = review_threads_nodes.map_or(0, |nodes| {
            nodes
                .iter()
                .filter(|n| n.get("isResolved").and_then(serde_json::Value::as_bool) == Some(false))
                .count() as u64
        });

        if let Some(obj) = raw_val.as_object_mut() {
            obj.insert("commentsCount".to_string(), serde_json::Value::from(comments_count));
            obj.insert("review_comments".to_string(), serde_json::Value::from(total_resolvable));
            obj.insert("unresolved_count".to_string(), serde_json::Value::from(unresolved));
            obj.insert(
                "total_resolvable_count".to_string(),
                serde_json::Value::from(total_resolvable),
            );

            if let Some(author_obj) = node.get("author").and_then(|a| a.as_object()) {
                obj.insert("author".to_string(), serde_json::Value::from(author_obj.clone()));
            }

            if let Some(repo_obj) = node.get("repository").and_then(|r| r.as_object()) {
                obj.insert("repository".to_string(), serde_json::Value::from(repo_obj.clone()));
            }

            // Flatten Connections (reviewRequests, latestReviews)
            for field in &["reviewRequests", "latestReviews"] {
                if let Some(nodes) = node.get(*field).and_then(|f| f.get("nodes")) {
                    obj.insert(field.to_string(), nodes.clone());
                }
            }
        }

        let raw: RawPullRequest = serde_json::from_value(raw_val.clone()).map_err(|e| {
            anyhow::anyhow!(
                "Failed to map GraphQL detail node to RawPullRequest: {e}. Node: {raw_val}"
            )
        })?;

        Ok(raw.into())
    }

    async fn fetch_check_runs(&self, repo: &str, ref_: &str) -> Result<Vec<CheckRun>> {
        // gh api repos/{owner}/{repo}/commits/{ref}/check-runs
        let path = format!("repos/{repo}/commits/{ref_}/check-runs");
        let output = self.run_gh(&["api", &path]).await?;

        let json: serde_json::Value = serde_json::from_str(&output)?;
        let check_runs_raw: Vec<RawCheckRun> = serde_json::from_value(json["check_runs"].clone())?;

        Ok(check_runs_raw
            .into_iter()
            .map(|r| CheckRun {
                name: r.name,
                status: r.status,
                conclusion: r.conclusion,
                url: r.url,
            })
            .collect())
    }

    async fn fetch_timeline(&self, repo: &str, pr_number: u32) -> Result<Vec<TimelineEvent>> {
        // gh api repos/{owner}/{repo}/issues/{number}/timeline
        let path = format!("repos/{repo}/issues/{pr_number}/timeline");
        let output = self.run_gh(&["api", &path]).await?;

        let raws: Vec<RawTimelineEvent> = serde_json::from_str(&output)?;

        Ok(raws.into_iter().map(map_timeline_event).collect())
    }

    async fn fetch_rate_limit(&self) -> Result<crate::domain::pr::RateLimitStatus> {
        let output = self.run_gh(&["api", "rate_limit"]).await?;
        let json: serde_json::Value = serde_json::from_str(&output)?;

        let core = &json["resources"]["core"];
        let status = crate::domain::pr::RateLimitStatus {
            remaining: core["remaining"].as_u64().unwrap_or(0) as u32,
            limit: core["limit"].as_u64().unwrap_or(0) as u32,
            reset_at: core["reset"].as_u64().unwrap_or(0),
        };

        self.rate_limit.update(&status);
        Ok(status)
    }

    async fn fetch_current_user(&self) -> Result<String> {
        let output = self.run_gh(&["api", "user", "--jq", ".login"]).await?;
        Ok(output.trim().to_string())
    }

    async fn open_pr_in_browser(&self, url: &str) -> Result<()> {
        self.run_gh(&["pr", "view", url, "--web"]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::{CIStatus, PRStatus, ReviewStatus};
    use crate::github::models::*;

    #[test]
    fn test_raw_to_pr_conversion() {
        let raw = RawPullRequest {
            id: "node_1".to_string(),
            number: 123,
            title: "Test PR".to_string(),
            author: RawAuthor { login: "alice".to_string() },
            repository: Some(RawRepository { name_with_owner: "org/repo".to_string() }),
            head_repository: None,
            state: "OPEN".to_string(),
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T11:00:00Z".to_string(),
            body: "Body text".to_string(),
            comments_count: Some(5),
            review_comments: Some(2),
            unresolved_count: Some(1),
            total_resolvable_count: Some(2),
            additions: Some(10),
            deletions: Some(5),
            review_decision: Some("APPROVED".to_string()),
            mergeable: Some("MERGEABLE".to_string()),
            merge_state_status: Some("CLEAN".to_string()),
            status_check_rollup: Some(RawStatusCheckRollup::Summary {
                state: "SUCCESS".to_string(),
            }),
            head_ref_oid: Some("sha123".to_string()),
            url: "https://github.com/org/repo/pull/123".to_string(),
            review_requests: Some(vec![RawReviewRequest {
                requested_reviewer: Some(RawRequestedReviewer {
                    typename: "User".to_string(),
                    login: Some("bob".to_string()),
                }),
            }]),
            latest_reviews: None,
            is_draft: false,
        };

        let pr: PullRequest = raw.into();

        assert_eq!(pr.number, 123);
        assert_eq!(pr.author, "alice");
        assert_eq!(pr.repo, "org/repo");
        assert_eq!(pr.status, PRStatus::Open);
        assert_eq!(pr.review_status, ReviewStatus::Approved);
        assert_eq!(pr.ci_status, CIStatus::Passing);
        assert_eq!(pr.comment_count, 7);
        assert_eq!(pr.unresolved_count, 1);
        assert_eq!(pr.total_resolvable_count, 2);
        assert_eq!(pr.conversational_count, 5);
        assert_eq!(pr.additions, 10);
        assert_eq!(pr.deletions, 5);
        assert_eq!(pr.head_ref, "sha123");
        assert_eq!(pr.requested_reviewers, vec!["bob".to_string()]);
    }

    #[test]
    fn test_deserialization_with_empty_status_rollup() {
        let json = r#"{
            "id": "PR_123",
            "number": 1,
            "title": "Test",
            "author": { "login": "alice" },
            "state": "OPEN",
            "createdAt": "2024-05-01T10:00:00Z",
            "updatedAt": "2024-05-01T10:00:00Z",
            "body": "Body",
            "url": "https://github.com/org/repo/pull/1",
            "statusCheckRollup": []
        }"#;

        let res: Result<RawPullRequest, _> = serde_json::from_str(json);
        assert!(res.is_ok(), "Should now succeed because statusCheckRollup can be an array");
        assert_eq!(res.unwrap().status_check_rollup.unwrap().state(), "EXPECTED");
    }

    #[test]
    fn test_repo_fallback_from_url() {
        let json = r#"{
            "id": "PR_123",
            "number": 1,
            "title": "Test",
            "author": { "login": "alice" },
            "state": "OPEN",
            "createdAt": "2024-05-01T10:00:00Z",
            "updatedAt": "2024-05-01T10:00:00Z",
            "body": "Body",
            "url": "https://github.com/kbjorklid/gh-notify-test/pull/1",
            "headRepository": { "nameWithOwner": "" }
        }"#;

        let raw: RawPullRequest = serde_json::from_str(json).unwrap();
        let pr: PullRequest = raw.into();
        assert_eq!(pr.repo, "kbjorklid/gh-notify-test");
    }

    #[test]
    fn test_parse_gh_search_prs_output() {
        let json = r#"[
            {
                "id": "PR_1",
                "number": 1,
                "title": "Fix bug",
                "author": { "login": "alice" },
                "repository": { "nameWithOwner": "org/repo" },
                "state": "OPEN",
                "createdAt": "2024-05-01T10:00:00Z",
                "updatedAt": "2024-05-01T11:00:00Z",
                "body": "Body",
                "commentsCount": 3,
                "url": "https://github.com/org/repo/pull/1"
            }
        ]"#;

        let raws: Vec<RawPullRequest> =
            serde_json::from_str(json).expect("Failed to parse sample JSON");
        assert_eq!(raws.len(), 1);
        let pr: PullRequest = raws[0].clone().into();
        assert_eq!(pr.number, 1);
        assert_eq!(pr.author, "alice");
        assert_eq!(pr.repo, "org/repo");
        assert_eq!(pr.comment_count, 3);
    }

    #[test]
    fn test_parse_user_provided_json() {
        let json = r#"[
  {
    "author": {
      "id": "MDQ6VXNlcjQ1NDUzNTk=",
      "is_bot": false,
      "login": "kbjorklid",
      "type": "User",
      "url": "https://github.com/kbjorklid"
    },
    "body": "This is an example PR for testing ghwatch.",
    "commentsCount": 0,
    "createdAt": "2026-05-10T10:51:43Z",
    "id": "PR_kwDOSZQ-mc7Z-0SB",
    "number": 1,
    "repository": {
      "name": "gh-notify-test",
      "nameWithOwner": "kbjorklid/gh-notify-test"
    },
    "state": "open",
    "title": "Example PR",
    "updatedAt": "2026-05-10T10:51:43Z",
    "url": "https://github.com/kbjorklid/gh-notify-test/pull/1"
  }
]"#;

        let raws: Vec<RawPullRequest> =
            serde_json::from_str(json).expect("Failed to parse user provided JSON");
        assert_eq!(raws.len(), 1);
        let pr: PullRequest = raws[0].clone().into();
        assert_eq!(pr.id, "PR_kwDOSZQ-mc7Z-0SB");
        assert_eq!(pr.number, 1);
        assert_eq!(pr.author, "kbjorklid");
        assert_eq!(pr.repo, "kbjorklid/gh-notify-test");
        assert_eq!(pr.status, crate::domain::pr::PRStatus::Open);
    }

    fn make_raw(event: &str) -> RawTimelineEvent {
        RawTimelineEvent {
            node_id: Some(format!("NODE_{event}")),
            typename: event.to_string(),
            actor: Some(crate::github::models::RawAuthor { login: "alice".to_string() }),
            created_at: Some("2024-01-01T10:00:00Z".to_string()),
            body: None,
            state: None,
            message: None,
            label: None,
        }
    }

    #[test]
    fn test_map_timeline_event_commented_maps_to_issue_comment() {
        let mut raw = make_raw("commented");
        raw.body = Some("LGTM".to_string());
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.event_type, "IssueComment");
        assert_eq!(ev.content.as_deref(), Some("LGTM"));
        assert_eq!(ev.actor, "alice");
        assert_eq!(ev.id, "NODE_commented");
        assert_eq!(ev.created_at, "2024-01-01T10:00:00Z");
    }

    #[test]
    fn test_map_timeline_event_reviewed_maps_to_pull_request_review() {
        let mut raw = make_raw("reviewed");
        raw.state = Some("approved".to_string());
        raw.body = Some("Nice work".to_string());
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.event_type, "PullRequestReview");
        assert_eq!(ev.content.as_deref(), Some("APPROVED: Nice work"));
    }

    #[test]
    fn test_map_timeline_event_reviewed_no_body() {
        let mut raw = make_raw("reviewed");
        raw.state = Some("commented".to_string());
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.event_type, "PullRequestReview");
        assert_eq!(ev.content.as_deref(), Some("COMMENTED"));
    }

    #[test]
    fn test_map_timeline_event_reviewed_state_uppercased() {
        let mut raw = make_raw("reviewed");
        raw.state = Some("changes_requested".to_string());
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.content.as_deref(), Some("CHANGES_REQUESTED"));
    }

    #[test]
    fn test_map_timeline_event_committed_maps_to_commit() {
        let mut raw = make_raw("committed");
        raw.actor = None;
        raw.created_at = None;
        raw.message = Some("Fix the bug".to_string());
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.event_type, "Commit");
        assert_eq!(ev.content.as_deref(), Some("Fix the bug"));
        assert_eq!(ev.actor, "unknown");
        assert_eq!(ev.created_at, "");
    }

    #[test]
    fn test_map_timeline_event_merged() {
        let ev = super::map_timeline_event(make_raw("merged"));
        assert_eq!(ev.event_type, "MergedEvent");
        assert_eq!(ev.content.as_deref(), Some("merged this pull request"));
    }

    #[test]
    fn test_map_timeline_event_closed() {
        let ev = super::map_timeline_event(make_raw("closed"));
        assert_eq!(ev.event_type, "ClosedEvent");
        assert_eq!(ev.content.as_deref(), Some("closed this pull request"));
    }

    #[test]
    fn test_map_timeline_event_reopened() {
        let ev = super::map_timeline_event(make_raw("reopened"));
        assert_eq!(ev.event_type, "ReopenedEvent");
        assert_eq!(ev.content.as_deref(), Some("reopened this pull request"));
    }

    #[test]
    fn test_map_timeline_event_labeled() {
        let mut raw = make_raw("labeled");
        raw.label = Some(crate::github::models::RawLabel { name: "bug".to_string() });
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.event_type, "LabeledEvent");
        assert_eq!(ev.content.as_deref(), Some("added label: bug"));
    }

    #[test]
    fn test_map_timeline_event_unlabeled() {
        let mut raw = make_raw("unlabeled");
        raw.label = Some(crate::github::models::RawLabel { name: "wip".to_string() });
        let ev = super::map_timeline_event(raw);
        assert_eq!(ev.event_type, "UnlabeledEvent");
        assert_eq!(ev.content.as_deref(), Some("removed label: wip"));
    }

    #[test]
    fn test_map_timeline_event_unknown_passes_through() {
        let ev = super::map_timeline_event(make_raw("review_requested"));
        assert_eq!(ev.event_type, "review_requested");
        assert!(ev.content.is_none());
    }
}

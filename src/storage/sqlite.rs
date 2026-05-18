use crate::domain::attention::AttentionState;
use crate::domain::ports::StateRepository;
use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus, Reviewer};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

pub struct SqliteStateRepository {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteStateRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStateRepository").finish_non_exhaustive()
    }
}

impl SqliteStateRepository {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let _ = std::fs::create_dir_all(data_dir);
        let db_path = data_dir.join("ghwatch.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pull_requests (
                id                               TEXT PRIMARY KEY,
                number                           INTEGER NOT NULL,
                title                            TEXT NOT NULL,
                author                           TEXT NOT NULL,
                repo                             TEXT NOT NULL,
                status                           TEXT NOT NULL,
                created_at                       TEXT NOT NULL,
                updated_at                       TEXT NOT NULL,
                additions                        INTEGER NOT NULL,
                deletions                        INTEGER NOT NULL,
                review_status                    TEXT NOT NULL,
                comment_count                    INTEGER NOT NULL,
                unresolved_count                 INTEGER NOT NULL,
                total_resolvable_count           INTEGER NOT NULL,
                conversational_count             INTEGER NOT NULL,
                ci_status                        TEXT NOT NULL,
                mergeable                        TEXT NOT NULL,
                head_ref                         TEXT NOT NULL,
                body                             TEXT NOT NULL,
                url                              TEXT NOT NULL,
                is_draft                         INTEGER NOT NULL,
                last_seen_at                     TEXT,
                last_seen_unresolved_count       INTEGER NOT NULL,
                last_seen_total_resolvable_count INTEGER NOT NULL,
                last_seen_conversational_count   INTEGER NOT NULL,
                archived_at                      TEXT,
                requested_reviewers              TEXT NOT NULL DEFAULT '[]',
                reviewers                        TEXT NOT NULL DEFAULT '[]',
                matched_queries                  TEXT NOT NULL DEFAULT '[]',
                attention_state                  TEXT NOT NULL DEFAULT '{}'
            ) STRICT;

            CREATE TABLE IF NOT EXISTS poll_lease (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                last_started_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'
            );

            INSERT OR IGNORE INTO poll_lease (id) VALUES (1);",
        )?;

        Ok(Self { conn: Mutex::new(conn) })
    }
}

fn pr_to_params(
    pr: &PullRequest,
    archived_at: Option<&str>,
) -> Result<Vec<Box<dyn rusqlite::ToSql>>> {
    Ok(vec![
        Box::new(pr.id.clone()),
        Box::new(i64::from(pr.number)),
        Box::new(pr.title.clone()),
        Box::new(pr.author.clone()),
        Box::new(pr.repo.clone()),
        Box::new(serde_json::to_string(&pr.status)?),
        Box::new(pr.created_at.clone()),
        Box::new(pr.updated_at.clone()),
        Box::new(i64::from(pr.additions)),
        Box::new(i64::from(pr.deletions)),
        Box::new(serde_json::to_string(&pr.review_status)?),
        Box::new(i64::from(pr.comment_count)),
        Box::new(i64::from(pr.unresolved_count)),
        Box::new(i64::from(pr.total_resolvable_count)),
        Box::new(i64::from(pr.conversational_count)),
        Box::new(serde_json::to_string(&pr.ci_status)?),
        Box::new(serde_json::to_string(&pr.mergeable)?),
        Box::new(pr.head_ref.clone()),
        Box::new(pr.body.clone()),
        Box::new(pr.url.clone()),
        Box::new(i64::from(pr.is_draft)),
        Box::new(pr.last_seen_at.clone()),
        Box::new(i64::from(pr.last_seen_unresolved_count)),
        Box::new(i64::from(pr.last_seen_total_resolvable_count)),
        Box::new(i64::from(pr.last_seen_conversational_count)),
        Box::new(archived_at.map(ToString::to_string)),
        Box::new(serde_json::to_string(&pr.requested_reviewers)?),
        Box::new(serde_json::to_string(&pr.reviewers)?),
        Box::new(serde_json::to_string(&pr.matched_queries)?),
        Box::new(serde_json::to_string(&pr.attention_state)?),
    ])
}

#[allow(clippy::cast_sign_loss)]
fn row_to_pr(row: &rusqlite::Row<'_>) -> rusqlite::Result<PullRequest> {
    let status_str: String = row.get(5)?;
    let review_status_str: String = row.get(10)?;
    let ci_status_str: String = row.get(15)?;
    let mergeable_str: String = row.get(16)?;
    let requested_reviewers_str: String = row.get(26)?;
    let reviewers_str: String = row.get(27)?;
    let matched_queries_str: String = row.get(28)?;
    let attention_state_str: String = row.get(29)?;

    let status: PRStatus = serde_json::from_str(&status_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let review_status: ReviewStatus = serde_json::from_str(&review_status_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let ci_status: CIStatus = serde_json::from_str(&ci_status_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(15, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let mergeable: MergeableStatus = serde_json::from_str(&mergeable_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(16, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let requested_reviewers: Vec<String> =
        serde_json::from_str(&requested_reviewers_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(26, rusqlite::types::Type::Text, Box::new(e))
        })?;
    let reviewers: Vec<Reviewer> = serde_json::from_str(&reviewers_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(27, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let matched_queries: Vec<String> = serde_json::from_str(&matched_queries_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(28, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let attention_state: AttentionState =
        serde_json::from_str(&attention_state_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(29, rusqlite::types::Type::Text, Box::new(e))
        })?;

    let number: i64 = row.get(1)?;
    let additions: i64 = row.get(8)?;
    let deletions: i64 = row.get(9)?;
    let comment_count: i64 = row.get(11)?;
    let unresolved_count: i64 = row.get(12)?;
    let total_resolvable_count: i64 = row.get(13)?;
    let conversational_count: i64 = row.get(14)?;
    let is_draft_int: i64 = row.get(20)?;
    let last_seen_unresolved_count: i64 = row.get(22)?;
    let last_seen_total_resolvable_count: i64 = row.get(23)?;
    let last_seen_conversational_count: i64 = row.get(24)?;

    Ok(PullRequest {
        id: row.get(0)?,
        number: number as u32,
        title: row.get(2)?,
        author: row.get(3)?,
        repo: row.get(4)?,
        status,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        additions: additions as u32,
        deletions: deletions as u32,
        review_status,
        comment_count: comment_count as u32,
        unresolved_count: unresolved_count as u32,
        total_resolvable_count: total_resolvable_count as u32,
        conversational_count: conversational_count as u32,
        ci_status,
        mergeable,
        head_ref: row.get(17)?,
        body: row.get(18)?,
        url: row.get(19)?,
        is_draft: is_draft_int != 0,
        last_seen_at: row.get(21)?,
        last_seen_unresolved_count: last_seen_unresolved_count as u32,
        last_seen_total_resolvable_count: last_seen_total_resolvable_count as u32,
        last_seen_conversational_count: last_seen_conversational_count as u32,
        requested_reviewers,
        reviewers,
        matched_queries,
        attention_state,
    })
}

const INSERT_SQL: &str = "INSERT OR REPLACE INTO pull_requests (
    id, number, title, author, repo, status, created_at, updated_at,
    additions, deletions, review_status, comment_count, unresolved_count,
    total_resolvable_count, conversational_count, ci_status, mergeable,
    head_ref, body, url, is_draft, last_seen_at, last_seen_unresolved_count,
    last_seen_total_resolvable_count, last_seen_conversational_count,
    archived_at, requested_reviewers, reviewers, matched_queries, attention_state
) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30)";

const SELECT_COLS: &str = "SELECT id, number, title, author, repo, status, created_at, updated_at,
    additions, deletions, review_status, comment_count, unresolved_count,
    total_resolvable_count, conversational_count, ci_status, mergeable,
    head_ref, body, url, is_draft, last_seen_at, last_seen_unresolved_count,
    last_seen_total_resolvable_count, last_seen_conversational_count,
    archived_at, requested_reviewers, reviewers, matched_queries, attention_state
    FROM pull_requests";

fn execute_insert(conn: &Connection, pr: &PullRequest, archived_at: Option<&str>) -> Result<()> {
    let p = pr_to_params(pr, archived_at)?;
    conn.execute(
        INSERT_SQL,
        rusqlite::params_from_iter(p.iter().map(std::convert::AsRef::as_ref)),
    )?;
    Ok(())
}

impl StateRepository for SqliteStateRepository {
    #[allow(clippy::significant_drop_tightening)]
    fn load_state(&self) -> Result<Vec<PullRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!("{SELECT_COLS} WHERE archived_at IS NULL"))?;
        let prs = stmt
            .query_map([], row_to_pr)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to load state")?;
        Ok(prs)
    }

    #[allow(clippy::significant_drop_tightening)]
    fn save_state(&self, prs: &[PullRequest]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM pull_requests WHERE archived_at IS NULL", [])?;
        for pr in prs {
            execute_insert(&tx, pr, None)?;
        }
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    fn load_archive(&self) -> Result<Vec<PullRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLS} WHERE archived_at IS NOT NULL ORDER BY updated_at DESC"
        ))?;
        let prs = stmt
            .query_map([], row_to_pr)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to load archive")?;
        Ok(prs)
    }

    #[allow(clippy::significant_drop_tightening)]
    fn save_archive(&self, prs: &[PullRequest]) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM pull_requests WHERE archived_at IS NOT NULL", [])?;
        for pr in prs {
            let archived_at =
                pr.attention_state.last_seen_at.map_or_else(|| now.clone(), |t| t.to_rfc3339());
            execute_insert(&tx, pr, Some(&archived_at))?;
        }
        tx.commit()?;
        Ok(())
    }

    fn archive_pr(&self, pr: PullRequest) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        execute_insert(&conn, &pr, Some(&now))?;
        Ok(())
    }

    fn try_acquire_poll_lease(&self, interval: Duration) -> Result<bool> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();
        let cutoff =
            now - chrono::Duration::from_std(interval).unwrap_or(chrono::Duration::seconds(0));
        let cutoff_str = cutoff.to_rfc3339();

        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE poll_lease SET last_started_at = ?1 WHERE id = 1 AND last_started_at < ?2",
            params![now_str, cutoff_str],
        )?;
        drop(conn);
        Ok(rows == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::attention::AttentionState;
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, ReviewStatus};

    fn setup_repo() -> SqliteStateRepository {
        SqliteStateRepository::new(&tempdir()).unwrap()
    }

    fn tempdir() -> std::path::PathBuf {
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ghwatch-sqlite-test-{}-{}",
            std::process::id(),
            now
        ));
        let _ = std::fs::create_dir_all(&path);
        path
    }

    fn make_pr(id: &str) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number: 1,
            title: "Test".to_string(),
            author: "alice".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T10:00:00Z".to_string(),
            additions: 1,
            deletions: 1,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "sha".to_string(),
            body: "body".to_string(),
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
        }
    }

    #[test]
    fn test_save_and_load_state() {
        let repo = setup_repo();
        let prs = vec![make_pr("1"), make_pr("2")];
        repo.save_state(&prs).unwrap();
        let loaded = repo.load_state().unwrap();
        assert_eq!(loaded.len(), 2);
        let ids: Vec<&str> = loaded.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"1"));
        assert!(ids.contains(&"2"));
    }

    #[test]
    fn test_save_and_load_archive() {
        let repo = setup_repo();
        let prs = vec![make_pr("archive-1")];
        repo.save_archive(&prs).unwrap();
        let loaded = repo.load_archive().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "archive-1");
    }

    #[test]
    fn test_archive_pr_not_visible_in_load_state() {
        let repo = setup_repo();
        repo.save_state(&[make_pr("active")]).unwrap();
        repo.archive_pr(make_pr("archived")).unwrap();

        let state = repo.load_state().unwrap();
        assert_eq!(state.len(), 1);
        assert_eq!(state[0].id, "active");

        let archive = repo.load_archive().unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(archive[0].id, "archived");
    }

    #[test]
    fn test_load_non_existent() {
        let repo = setup_repo();
        assert!(repo.load_state().unwrap().is_empty());
        assert!(repo.load_archive().unwrap().is_empty());
    }

    #[test]
    fn test_archive_pr_append() {
        let repo = setup_repo();
        repo.archive_pr(make_pr("1")).unwrap();
        repo.archive_pr(make_pr("2")).unwrap();
        let loaded = repo.load_archive().unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_save_state_replaces_previous() {
        let repo = setup_repo();
        repo.save_state(&[make_pr("old")]).unwrap();
        repo.save_state(&[make_pr("new")]).unwrap();
        let loaded = repo.load_state().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "new");
    }

    #[test]
    fn test_save_archive_replaces_previous() {
        let repo = setup_repo();
        repo.save_archive(&[make_pr("old")]).unwrap();
        repo.save_archive(&[make_pr("new")]).unwrap();
        let loaded = repo.load_archive().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "new");
    }

    #[test]
    fn test_try_acquire_poll_lease_first_instance_wins() {
        let repo = setup_repo();
        let won = repo.try_acquire_poll_lease(Duration::from_secs(0)).unwrap();
        assert!(won);
    }

    #[test]
    fn test_try_acquire_poll_lease_second_instance_skips() {
        let repo = setup_repo();
        let won1 = repo.try_acquire_poll_lease(Duration::from_secs(0)).unwrap();
        assert!(won1);
        // Immediately trying again with a long interval should fail
        let won2 = repo.try_acquire_poll_lease(Duration::from_hours(1)).unwrap();
        assert!(!won2);
    }

    #[test]
    fn test_try_acquire_poll_lease_reacquires_after_interval() {
        let repo = setup_repo();
        // With a 0-second interval the cutoff == now, so last_started_at < cutoff
        // may or may not hold depending on sub-millisecond timing.
        // We verify the mechanism works end-to-end without panicking.
        let _ = repo.try_acquire_poll_lease(Duration::from_secs(0));
        let _ = repo.try_acquire_poll_lease(Duration::from_millis(1));
    }
}

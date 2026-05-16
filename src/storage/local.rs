use crate::domain::ports::StateRepository;
use crate::domain::pr::PullRequest;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct FileStateRepository {
    state_path: PathBuf,
    archive_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct StateFile {
    prs: Vec<PullRequest>,
}

impl FileStateRepository {
    #[must_use]
    pub fn new(data_dir: &std::path::Path) -> Self {
        let state_path = data_dir.join("state.toml");
        let archive_path = data_dir.join("archive.toml");

        let _ = fs::create_dir_all(data_dir);

        Self { state_path, archive_path }
    }
}

impl StateRepository for FileStateRepository {
    fn load_state(&self) -> Result<Vec<PullRequest>> {
        if !self.state_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.state_path).context("Failed to read state file")?;

        let state: StateFile = toml::from_str(&content).context("Failed to parse state file")?;

        Ok(state.prs)
    }

    fn save_state(&self, prs: &[PullRequest]) -> Result<()> {
        let state = StateFile { prs: prs.to_vec() };

        let content = toml::to_string(&state).context("Failed to serialize state")?;

        fs::write(&self.state_path, content).context("Failed to write state file")?;

        Ok(())
    }

    fn load_archive(&self) -> Result<Vec<PullRequest>> {
        let mut all_prs = Vec::new();

        // Load from archive.toml, archive.1.toml, archive.2.toml
        let paths = vec![
            self.archive_path.clone(),
            self.archive_path.with_file_name("archive.1.toml"),
            self.archive_path.with_file_name("archive.2.toml"),
        ];

        for path in paths {
            if path.exists()
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(archive) = toml::from_str::<StateFile>(&content)
            {
                all_prs.extend(archive.prs);
            }
        }

        // Sort by updated_at descending
        all_prs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(all_prs)
    }

    fn save_archive(&self, archive: &[PullRequest]) -> Result<()> {
        let archive_data = StateFile { prs: archive.to_vec() };

        let content = toml::to_string(&archive_data).context("Failed to serialize archive")?;

        fs::write(&self.archive_path, content).context("Failed to write archive file")?;

        // Clear rotated files to ensure deletion is complete
        let rotated = vec![
            self.archive_path.with_file_name("archive.1.toml"),
            self.archive_path.with_file_name("archive.2.toml"),
        ];
        for path in rotated {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }

        Ok(())
    }

    fn archive_pr(&self, pr: PullRequest) -> Result<()> {
        use std::io::Write;

        if self.archive_path.exists()
            && let Ok(metadata) = fs::metadata(&self.archive_path)
            && metadata.len() > 1024 * 1024
        {
            // 1 MB
            crate::storage::archive::rotate(&self.archive_path)?;
        }

        let archive_data = StateFile { prs: vec![pr] };

        let content =
            toml::to_string(&archive_data).context("Failed to serialize archive entry")?;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.archive_path)
            .context("Failed to open archive file for appending")?;

        file.write_all(content.as_bytes())?;
        file.write_all(b"\n")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, ReviewStatus};

    fn create_test_pr(id: &str) -> PullRequest {
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
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        }
    }

    fn setup_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        path.push(format!("ghwatch-test-{}-{}", std::process::id(), now));
        let _ = fs::create_dir_all(&path);
        path
    }

    #[test]
    fn test_save_and_load_state() {
        let dir = setup_temp_dir();
        let repo = FileStateRepository::new(&dir);

        let prs = vec![create_test_pr("1"), create_test_pr("2")];
        repo.save_state(&prs).unwrap();

        let loaded = repo.load_state().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "1");
        assert_eq!(loaded[1].id, "2");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_save_and_load_archive() {
        let dir = setup_temp_dir();
        let repo = FileStateRepository::new(&dir);

        let prs = vec![create_test_pr("archive-1")];
        repo.save_archive(&prs).unwrap();

        let loaded = repo.load_archive().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "archive-1");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_archive_pr_append() {
        let dir = setup_temp_dir();
        let repo = FileStateRepository::new(&dir);

        repo.archive_pr(create_test_pr("1")).unwrap();
        repo.archive_pr(create_test_pr("2")).unwrap();

        let loaded = repo.load_archive().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "1");
        assert_eq!(loaded[1].id, "2");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_load_non_existent() {
        let dir = setup_temp_dir();
        let repo = FileStateRepository::new(&dir);

        let loaded = repo.load_state().unwrap();
        assert!(loaded.is_empty());

        let loaded_archive = repo.load_archive().unwrap();
        assert!(loaded_archive.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_save_archive_clears_rotated() {
        let dir = setup_temp_dir();
        let repo = FileStateRepository::new(&dir);

        let archive_1 = dir.join("archive.1.toml");
        fs::write(&archive_1, "prs = []").unwrap();
        assert!(archive_1.exists());

        repo.save_archive(&[create_test_pr("new")]).unwrap();

        assert!(!archive_1.exists());
        let loaded = repo.load_archive().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "new");
        let _ = fs::remove_dir_all(dir);
    }
}

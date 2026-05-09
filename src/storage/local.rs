use anyhow::{Result, Context};
use std::fs;
use std::path::PathBuf;
use crate::domain::pr::PullRequest;
use crate::domain::ports::StateRepository;
use serde::{Deserialize, Serialize};

pub struct FileStateRepository {
    state_path: PathBuf,
    archive_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct StateFile {
    prs: Vec<PullRequest>,
}

impl FileStateRepository {
    pub fn new(data_dir: PathBuf) -> Self {
        let state_path = data_dir.join("state.toml");
        let archive_path = data_dir.join("archive.toml");
        
        // Ensure data dir exists
        let _ = fs::create_dir_all(&data_dir);
        
        Self {
            state_path,
            archive_path,
        }
    }
}

impl StateRepository for FileStateRepository {
    fn load_state(&self) -> Result<Vec<PullRequest>> {
        if !self.state_path.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&self.state_path)
            .context("Failed to read state file")?;
        
        let state: StateFile = toml::from_str(&content)
            .context("Failed to parse state file")?;
        
        Ok(state.prs)
    }

    fn save_state(&self, prs: &[PullRequest]) -> Result<()> {
        let state = StateFile {
            prs: prs.to_vec(),
        };
        
        let content = toml::to_string(&state)
            .context("Failed to serialize state")?;
        
        fs::write(&self.state_path, content)
            .context("Failed to write state file")?;
        
        Ok(())
    }

    fn load_archive(&self) -> Result<Vec<PullRequest>> {
        if !self.archive_path.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&self.archive_path)
            .context("Failed to read archive file")?;
        
        let archive: StateFile = toml::from_str(&content)
            .context("Failed to parse archive file")?;
        
        Ok(archive.prs)
    }

    fn save_archive(&self, prs: &[PullRequest]) -> Result<()> {
        let archive = StateFile {
            prs: prs.to_vec(),
        };
        
        let content = toml::to_string(&archive)
            .context("Failed to serialize archive")?;
        
        // Check size before writing to trigger rotation
        if self.archive_path.exists()
            && let Ok(metadata) = fs::metadata(&self.archive_path)
            && metadata.len() > 1024 * 1024 { // 1 MB
                crate::storage::archive::rotate(&self.archive_path)?;
        }

        fs::write(&self.archive_path, content)
            .context("Failed to write archive file")?;
        
        Ok(())
    }
}

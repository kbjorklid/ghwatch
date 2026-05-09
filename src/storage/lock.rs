use anyhow::{Result, Context};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;

pub struct FileLock {
    file: File,
}

impl FileLock {
    pub fn acquire_exclusive(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .context("Failed to open lock file")?;

        file.try_lock_exclusive().context("Failed to acquire exclusive lock")?;

        Ok(Self { file })
    }

    pub fn acquire_shared(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .context("Failed to open lock file")?;

        file.try_lock_shared().context("Failed to acquire shared lock")?;

        Ok(Self { file })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

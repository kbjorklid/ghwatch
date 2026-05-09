use anyhow::{Result, Context};
use std::fs;
use std::path::Path;

pub fn rotate(archive_path: &Path) -> Result<()> {
    // archive.toml -> archive.1.toml -> archive.2.toml
    let data_dir = archive_path.parent().context("No parent directory for archive")?;
    
    let path2 = data_dir.join("archive.2.toml");
    let path1 = data_dir.join("archive.1.toml");
    
    // 3 files max means we keep archive.toml, archive.1.toml, archive.2.toml
    // If archive.2.toml exists, it will be overwritten by archive.1.toml
    
    if path1.exists() {
        fs::rename(&path1, &path2).context("Failed to rename archive.1 to archive.2")?;
    }
    
    if archive_path.exists() {
        fs::rename(archive_path, &path1).context("Failed to rename archive to archive.1")?;
    }
    
    Ok(())
}

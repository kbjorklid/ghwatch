use anyhow::{Context, Result};
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        path.push(format!("ghwatch-archive-test-{}-{}", std::process::id(), now));
        let _ = fs::create_dir_all(&path);
        path
    }

    #[test]
    fn test_rotation() {
        let dir = setup_temp_dir();
        let archive = dir.join("archive.toml");

        fs::write(&archive, "current").unwrap();
        rotate(&archive).unwrap();

        assert!(!archive.exists());
        assert!(dir.join("archive.1.toml").exists());
        assert_eq!(fs::read_to_string(dir.join("archive.1.toml")).unwrap(), "current");

        fs::write(&archive, "new").unwrap();
        rotate(&archive).unwrap();

        assert!(!archive.exists());
        assert!(dir.join("archive.1.toml").exists());
        assert!(dir.join("archive.2.toml").exists());
        assert_eq!(fs::read_to_string(dir.join("archive.1.toml")).unwrap(), "new");
        assert_eq!(fs::read_to_string(dir.join("archive.2.toml")).unwrap(), "current");

        let _ = fs::remove_dir_all(dir);
    }
}

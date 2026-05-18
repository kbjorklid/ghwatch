use directories::ProjectDirs;
use std::path::PathBuf;

pub mod sqlite;

#[must_use]
pub fn get_data_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "ghwatch", "ghwatch").map(|dirs| dirs.data_dir().to_path_buf())
}

#[must_use]
pub fn get_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "ghwatch", "ghwatch").map(|dirs| dirs.config_dir().to_path_buf())
}

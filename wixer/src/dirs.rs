use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

pub fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("com", "natyamatsya", "wixer").context("Could not determine home directory")
}

pub fn get_cache_dir() -> Result<PathBuf> {
    let proj_dirs = get_project_dirs()?;
    Ok(proj_dirs.cache_dir().to_path_buf())
}

pub fn get_config_dir() -> Result<PathBuf> {
    let proj_dirs = get_project_dirs()?;
    Ok(proj_dirs.config_dir().to_path_buf())
}

pub fn get_data_dir() -> Result<PathBuf> {
    let proj_dirs = get_project_dirs()?;
    Ok(proj_dirs.data_dir().to_path_buf())
}

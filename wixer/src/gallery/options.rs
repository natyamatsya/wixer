use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Yaml,
    Submodule,
    All,
}

#[derive(Clone)]
pub struct ResolvedPaths {
    pub(super) source: PathBuf,
    pub(super) output: PathBuf,
    pub(super) yaml_gallery: PathBuf,
    pub(super) configs_dir: PathBuf,
    pub(super) v4_dir: PathBuf,
    pub(super) msi_dir: PathBuf,
    pub(super) msix_dir: PathBuf,
    pub(super) msix_tmp_dir: PathBuf,
}

#[derive(Clone)]
pub struct GalleryOptions {
    pub source_type: SourceType,
    pub source: PathBuf,
    pub output: PathBuf,
    pub configs: PathBuf,
    pub yaml_gallery: PathBuf,
    pub clean: bool,
    pub only_prefixes: Option<Vec<String>>,
    pub upgrade: bool,
    pub build: bool,
    pub refresh: bool,
    pub wixer: PathBuf,
    pub verbose: bool,
    pub validate: bool,
    pub msix: bool,
}

impl GalleryOptions {
    pub fn resolve_paths(&self) -> Result<ResolvedPaths> {
        let cwd = std::env::current_dir()?;

        let source = if self.source.is_absolute() {
            self.source.clone()
        } else {
            cwd.join(&self.source)
        };

        let output = if self.output.is_absolute() {
            self.output.clone()
        } else {
            cwd.join(&self.output)
        };

        let yaml_gallery = if self.yaml_gallery.is_absolute() {
            self.yaml_gallery.clone()
        } else {
            cwd.join(&self.yaml_gallery)
        };

        let configs_dir = if self.configs.is_absolute() {
            self.configs.clone()
        } else {
            cwd.join(&self.configs)
        };

        let v4_dir = output.join("wix-v4");
        let msi_dir = output.join("msi");
        let msix_dir = output.join("msix");
        let msix_tmp_dir = output.join(".msix-tmp");

        Ok(ResolvedPaths {
            source,
            output,
            yaml_gallery,
            configs_dir,
            v4_dir,
            msi_dir,
            msix_dir,
            msix_tmp_dir,
        })
    }
}

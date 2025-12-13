use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub(super) fn copy_supporting_files(src: &Path, dst: &Path) -> Result<()> {
    let supporting_extensions = ["md", "txt", "ps1", "cmd", "bat", "json"];

    for entry in WalkDir::new(src).max_depth(1) {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if supporting_extensions.contains(&ext) {
            let filename = path.file_name().unwrap();
            let dst_path = dst.join(filename);
            fs::copy(path, &dst_path)
                .with_context(|| format!("Failed to copy {}", path.display()))?;
        }
    }

    Ok(())
}

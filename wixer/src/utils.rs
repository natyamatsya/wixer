use std::env;
use std::path::PathBuf;

pub fn find_wixer_root() -> std::io::Result<PathBuf> {
    let current_dir = env::current_dir()?;
    let mut dir = current_dir.as_path();

    loop {
        let wixer_dir = dir.join(".wixer");
        if wixer_dir.exists() && wixer_dir.is_dir() {
            return Ok(wixer_dir);
        }

        if let Some(parent) = dir.parent() {
            dir = parent;
        } else {
            break;
        }
    }

    Ok(current_dir.join(".wixer"))
}

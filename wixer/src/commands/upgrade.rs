//! WiX v3 to v4 upgrade command
//!
//! This module provides the CLI interface for the upgrade functionality.
//! The actual transformation logic is in the `upgrade` module.

use anyhow::Result;
use std::path::PathBuf;

use crate::upgrade;

/// Upgrade a WiX v3 file to v4
///
/// # Arguments
/// * `input` - Path to the input WiX v3 file
/// * `output` - Optional output path (defaults to overwriting input)
pub fn run(input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let output_path = output.unwrap_or_else(|| input.clone());
    upgrade::upgrade_file(&input, &output_path)
}

/// Upgrade multiple WiX v3 files to v4 with shared cross-file context
///
/// This enables transformations that depend on information from other files,
/// such as WIXUI_INSTALLDIR property in one file and UIRef in another.
///
/// # Arguments
/// * `files` - List of (input, output) path pairs
pub fn run_batch(files: &[(PathBuf, PathBuf)]) -> Result<()> {
    upgrade::upgrade_files(files)
}

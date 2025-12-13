use anyhow::Result;
use std::fs;

use crate::commands::import;
use crate::common::AttributeStrategy;
use crate::project::WixProject;

use super::Stats;
use super::discover;
use super::fs_support;
use super::index;
use super::options::{GalleryOptions, ResolvedPaths};

pub(super) fn run_upgrade(
    options: &GalleryOptions,
    paths: &ResolvedPaths,
    stats: &mut Stats,
) -> Result<()> {
    if !paths.source.exists() {
        eprintln!(
            "Warning: Submodule source not found: {}",
            paths.source.display()
        );
        return Ok(());
    }

    fs::create_dir_all(&paths.v4_dir)?;
    let submodule_examples =
        discover::find_submodule_examples(&paths.source, options.only_prefixes.as_ref())?;

    println!("=== Upgrading Submodule Examples (v3 → v4) ===");
    println!("Found {} examples", submodule_examples.len());
    println!();

    for example in &submodule_examples {
        print!("Upgrading {}... ", example.name);

        let v4_example_dir = paths.v4_dir.join(&example.name);
        fs::create_dir_all(&v4_example_dir)?;
        fs_support::copy_supporting_files(&example.path, &v4_example_dir)?;

        let mut project = WixProject::new();
        for wxs_file in &example.wxs_files {
            let _ = project.add_file(wxs_file);
        }

        match project.upgrade_to_v4_dir(&example.path, &v4_example_dir) {
            Ok(paths) => {
                println!("✓ ({} files)", paths.len());
                stats.upgraded += paths.len();
            }
            Err(e) => {
                println!("✗ ({})", e);
                stats.upgrade_errors += 1;
            }
        }

        if options.refresh {
            let yaml_example_dir = paths.yaml_gallery.join(&example.name);
            fs::create_dir_all(&yaml_example_dir)?;
            fs_support::copy_supporting_files(&example.path, &yaml_example_dir)?;
            let _ = import::run(
                &v4_example_dir,
                &yaml_example_dir,
                AttributeStrategy::Inline,
            );
        }
    }

    println!();
    println!(
        "Upgraded: {} files ({} errors)",
        stats.upgraded, stats.upgrade_errors
    );

    index::generate_index(&paths.output, &submodule_examples)?;

    Ok(())
}

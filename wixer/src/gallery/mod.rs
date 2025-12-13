mod build;
mod discover;
mod fs_support;
mod index;
mod msix;
use anyhow::Result;
pub mod options;
mod upgrade;

use std::fs;

use options::GalleryOptions;

pub fn run_with_options(options: GalleryOptions) -> Result<()> {
    let paths = options.resolve_paths()?;

    if options.clean {
        for dir in [
            &paths.v4_dir,
            &paths.msi_dir,
            &paths.msix_dir,
            &paths.msix_tmp_dir,
        ] {
            if dir.exists() {
                println!("Cleaning {}", dir.display());
                fs::remove_dir_all(dir)?;
            }
        }
    }

    let mut stats = Stats::default();

    if options.upgrade
        && (options.source_type == options::SourceType::Submodule
            || options.source_type == options::SourceType::All)
    {
        upgrade::run_upgrade(&options, &paths, &mut stats)?;
    }

    if options.build {
        build::run_build(&options, &paths, &mut stats)?;
    }

    if options.msix {
        msix::run_msix(&options, &paths, &mut stats)?;
    }

    let has_errors = stats.upgrade_errors > 0 || stats.build_errors > 0;
    if has_errors {
        anyhow::bail!("Completed with errors");
    }

    Ok(())
}

#[derive(Default)]
struct Stats {
    upgraded: usize,
    upgrade_errors: usize,
    built: usize,
    build_errors: usize,
}

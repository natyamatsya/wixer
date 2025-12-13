use anyhow::Result;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::Stats;
use super::discover;
use super::discover::BuildExample;
use super::options::{GalleryOptions, ResolvedPaths};

enum BuildResult {
    Success,
    ExpectedFailure,
    Skipped(String),
    Error(String),
}

pub(super) fn run_build(
    options: &GalleryOptions,
    paths: &ResolvedPaths,
    stats: &mut Stats,
) -> Result<()> {
    fs::create_dir_all(&paths.msi_dir)?;

    let build_examples = discover::discover_build_examples(
        &options.source_type,
        &paths.yaml_gallery,
        &paths.v4_dir,
        &paths.configs_dir,
        options.only_prefixes.as_ref(),
    )?;

    if build_examples.is_empty() {
        println!("No buildable examples found");
        return Ok(());
    }

    let total = build_examples.len();
    let jobs = rayon::current_num_threads();
    println!();
    println!(
        "=== Building MSI Packages ({} examples, {} parallel jobs) ===",
        total, jobs
    );

    let built = AtomicUsize::new(0);
    let expected_failed = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let errors = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);
    let job_id = AtomicUsize::new(0);
    let output_lines = Mutex::new(Vec::new());

    build_examples.par_iter().for_each(|example| {
        let my_job_id = job_id.fetch_add(1, Ordering::Relaxed);
        let result = build_example(
            example,
            &paths.msi_dir,
            &options.wixer,
            options.verbose,
            my_job_id,
        );
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;

        let line = match result {
            BuildResult::Success => {
                built.fetch_add(1, Ordering::Relaxed);
                format!("[{}/{}] Building {}... ✓", done, total, example.name)
            }
            BuildResult::ExpectedFailure => {
                expected_failed.fetch_add(1, Ordering::Relaxed);
                format!(
                    "[{}/{}] Building {}... ✓ (expected failure)",
                    done, total, example.name
                )
            }
            BuildResult::Skipped(reason) => {
                skipped.fetch_add(1, Ordering::Relaxed);
                format!(
                    "[{}/{}] Building {}... ⊘ ({})",
                    done, total, example.name, reason
                )
            }
            BuildResult::Error(msg) => {
                errors.fetch_add(1, Ordering::Relaxed);
                format!(
                    "[{}/{}] Building {}... ✗ ({})",
                    done, total, example.name, msg
                )
            }
        };

        println!("{}", line);
        output_lines
            .lock()
            .unwrap()
            .push((example.name.clone(), line));
    });

    stats.built = built.load(Ordering::Relaxed);
    stats.build_errors = errors.load(Ordering::Relaxed);

    println!();
    println!(
        "MSI packages: {} built, {} expected failures, {} skipped, {} failed",
        stats.built,
        expected_failed.load(Ordering::Relaxed),
        skipped.load(Ordering::Relaxed),
        stats.build_errors
    );
    println!("Output: {}", paths.msi_dir.display());

    if options.validate && stats.built > 0 {
        run_ice_validation(&paths.msi_dir, stats)?;
    }

    Ok(())
}

fn run_ice_validation(msi_dir: &Path, stats: &mut Stats) -> Result<()> {
    println!();

    let msi_files: Vec<_> = fs::read_dir(msi_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "msi").unwrap_or(false))
        .collect();

    let total = msi_files.len();
    let jobs = rayon::current_num_threads();
    println!(
        "=== Running ICE Validation ({} files, {} parallel jobs) ===",
        total, jobs
    );

    let validation_errors = AtomicUsize::new(0);
    let validation_warnings = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);

    msi_files.par_iter().for_each(|path| {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let output = Command::new("wix")
            .args(["msi", "validate", path.to_str().unwrap()])
            .output();

        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;

        let line = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{}{}", stdout, stderr);

                let errors: Vec<&str> = combined
                    .lines()
                    .filter(|l| l.contains("error WIX"))
                    .collect();
                let warnings: Vec<&str> = combined
                    .lines()
                    .filter(|l| l.contains("warning WIX"))
                    .collect();

                if errors.is_empty() && warnings.is_empty() {
                    format!("[{}/{}] Validating {}... ✓", done, total, name)
                } else if errors.is_empty() {
                    validation_warnings.fetch_add(warnings.len(), Ordering::Relaxed);
                    format!(
                        "[{}/{}] Validating {}... ⚠ ({} warnings)",
                        done,
                        total,
                        name,
                        warnings.len()
                    )
                } else {
                    validation_errors.fetch_add(errors.len(), Ordering::Relaxed);
                    validation_warnings.fetch_add(warnings.len(), Ordering::Relaxed);
                    let error_details: String =
                        errors.iter().map(|e| format!("\n  {}", e)).collect();
                    format!(
                        "[{}/{}] Validating {}... ✗ ({} errors, {} warnings){}",
                        done,
                        total,
                        name,
                        errors.len(),
                        warnings.len(),
                        error_details
                    )
                }
            }
            Err(e) => format!(
                "[{}/{}] Validating {}... ✗ (wix not found: {})",
                done, total, name, e
            ),
        };

        println!("{}", line);
    });

    let validation_errors = validation_errors.load(Ordering::Relaxed);
    let validation_warnings = validation_warnings.load(Ordering::Relaxed);

    println!();
    println!(
        "Validation: {} errors, {} warnings",
        validation_errors, validation_warnings
    );

    if validation_errors > 0 {
        stats.build_errors += 1;
    }

    Ok(())
}

fn build_example(
    example: &BuildExample,
    msi_dir: &Path,
    wixer_path: &Path,
    _verbose: bool,
    job_id: usize,
) -> BuildResult {
    if let Some(ref config) = example.config
        && !config.buildable
    {
        return BuildResult::Skipped(
            config
                .skip_reason
                .clone()
                .unwrap_or_else(|| "not buildable".to_string()),
        );
    }

    let msi_path = msi_dir.join(format!("{}.msi", example.name));

    if let Some(ref config) = example.config {
        for step in &config.prebuild {
            if step.produces.is_empty() {
                continue;
            }

            let missing_outputs: Vec<String> = step
                .produces
                .iter()
                .filter(|p| !example.source_dir.join(p).exists())
                .cloned()
                .collect();

            if missing_outputs.is_empty() {
                continue;
            }

            let mut pre = Command::new(&step.tool);
            pre.current_dir(
                step.cwd
                    .as_deref()
                    .map(|p| example.source_dir.join(p))
                    .unwrap_or_else(|| example.source_dir.clone()),
            );
            pre.args(&step.args);

            if _verbose {
                eprintln!("  Prebuild ({}): {:?}", example.name, pre);
            }

            match pre.output() {
                Ok(out) => {
                    if !out.status.success() {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let combined = format!("{}\n{}", stderr, stdout);
                        let first_error = combined
                            .lines()
                            .find(|l| l.contains("error") || l.contains("failed"))
                            .unwrap_or("prebuild failed");
                        return BuildResult::Error(first_error.chars().take(80).collect());
                    }
                }
                Err(e) => {
                    return BuildResult::Error(format!("prebuild exec failed: {}", e));
                }
            }

            let still_missing: Vec<String> = step
                .produces
                .iter()
                .filter(|p| !example.source_dir.join(p).exists())
                .cloned()
                .collect();
            if !still_missing.is_empty() {
                return BuildResult::Error(format!(
                    "missing outputs: {}",
                    still_missing.join(", ")
                ));
            }
        }
    }

    let mut cmd = Command::new(wixer_path);
    cmd.current_dir(&example.source_dir);
    cmd.env("WIXER_BUILD_ID", format!("{}", job_id));
    cmd.arg("build");

    for file in &example.build_files {
        let fname = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.ends_with(".wxs.yaml") || fname.ends_with(".wxs") {
            let rel_path = file.strip_prefix(&example.source_dir).unwrap_or(file);
            cmd.arg(rel_path);
        }
    }

    if let Some(ref config) = example.config {
        cmd.args(config.build_args());
    }

    cmd.args(["-o", msi_path.to_str().unwrap_or_default()]);

    if _verbose {
        eprintln!("  Running: {:?}", cmd);
    }

    match cmd.output() {
        Ok(output) => {
            let expect_failure = example
                .config
                .as_ref()
                .is_some_and(|c| c.expect_build_failure);

            if output.status.success() {
                if expect_failure {
                    BuildResult::Error("build succeeded but failure was expected".to_string())
                } else {
                    BuildResult::Success
                }
            } else if expect_failure {
                BuildResult::ExpectedFailure
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let combined = format!("{}\n{}", stderr, stdout);

                let first_error = combined
                    .lines()
                    .find(|l| l.contains("error WIX") || l.contains("error:"))
                    .unwrap_or("build failed");
                BuildResult::Error(first_error.chars().take(80).collect())
            }
        }
        Err(e) => BuildResult::Error(format!("exec failed: {}", e)),
    }
}

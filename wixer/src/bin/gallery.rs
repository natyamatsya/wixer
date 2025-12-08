//! Gallery Builder - Builds the wixer example gallery
//!
//! This tool processes examples from multiple sources:
//! - WiXInstallerExamples submodule (v3 → v4 upgrade)
//! - Curated wixer-yaml gallery (native YAML examples)
//!
//! Usage:
//!   wixer-gallery --build --source-type yaml    # Build only curated YAML examples
//!   wixer-gallery --build --source-type all     # Build from all sources
//!   wixer-gallery --upgrade                     # Upgrade submodule v3 → v4

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use walkdir::WalkDir;

use wixer::commands::import;
use wixer::common::AttributeStrategy;
use wixer::example_config::ExampleConfig;
use wixer::project::WixProject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SourceType {
    /// Only curated YAML examples (wixer-yaml/)
    Yaml,
    /// Only upgraded submodule examples (WiXInstallerExamples → wix-v4/)
    Submodule,
    /// All examples from both sources
    All,
}

#[derive(Parser)]
#[command(name = "wixer-gallery")]
#[command(about = "Build the wixer example gallery")]
struct Args {
    /// Source type to process
    #[arg(long, default_value = "all")]
    source_type: SourceType,

    /// Source directory containing WiX v3 examples (submodule)
    #[arg(short, long, default_value = "examples/WiXInstallerExamples")]
    source: PathBuf,

    /// Output directory for generated files (wix-v4/, msi/)
    #[arg(short, long, default_value = "examples/gallery")]
    output: PathBuf,

    /// Directory containing example config files (*.yaml)
    #[arg(short, long, default_value = "examples/configs")]
    configs: PathBuf,

    /// Curated YAML gallery directory (tracked in git)
    #[arg(long, default_value = "examples/wixer-yaml")]
    yaml_gallery: PathBuf,

    /// Clean output directories before building
    #[arg(long)]
    clean: bool,

    /// Only process specific examples (comma-separated prefixes, e.g., "00,01,02")
    #[arg(long)]
    only: Option<String>,

    /// Upgrade submodule examples from v3 to v4
    #[arg(long)]
    upgrade: bool,

    /// Build MSI packages
    #[arg(short, long)]
    build: bool,

    /// Refresh curated YAML gallery from upgraded v4 files
    #[arg(long)]
    refresh: bool,

    /// Path to wixer executable (for --build)
    #[arg(long, default_value = "wixer")]
    wixer: PathBuf,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Run ICE validation on built MSI files
    #[arg(long)]
    validate: bool,

    /// Number of parallel jobs (0 = auto)
    #[arg(short = 'j', long, default_value = "0")]
    jobs: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    // Configure thread pool
    if args.jobs > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.jobs)
            .build_global()
            .ok();
    }

    let cwd = std::env::current_dir()?;
    let source = if args.source.is_absolute() { args.source.clone() } else { cwd.join(&args.source) };
    let output = if args.output.is_absolute() { args.output.clone() } else { cwd.join(&args.output) };
    let yaml_gallery = if args.yaml_gallery.is_absolute() { args.yaml_gallery.clone() } else { cwd.join(&args.yaml_gallery) };
    let configs_dir = if args.configs.is_absolute() { args.configs.clone() } else { cwd.join(&args.configs) };

    let v4_dir = output.join("wix-v4");
    let msi_dir = output.join("msi");

    // Parse filter
    let filter: Option<Vec<String>> = args.only.map(|s| {
        s.split(',').map(|p| p.trim().to_string()).collect()
    });

    // Clean if requested
    if args.clean {
        for dir in [&v4_dir, &msi_dir] {
            if dir.exists() {
                println!("Cleaning {}", dir.display());
                fs::remove_dir_all(dir)?;
            }
        }
    }

    let mut stats = Stats::default();

    // === UPGRADE: Process submodule examples (v3 → v4) ===
    if args.upgrade && (args.source_type == SourceType::Submodule || args.source_type == SourceType::All) {
        if !source.exists() {
            eprintln!("Warning: Submodule source not found: {}", source.display());
        } else {
            fs::create_dir_all(&v4_dir)?;
            let submodule_examples = find_submodule_examples(&source, filter.as_ref())?;
            
            println!("=== Upgrading Submodule Examples (v3 → v4) ===");
            println!("Found {} examples", submodule_examples.len());
            println!();

            for example in &submodule_examples {
                print!("Upgrading {}... ", example.name);
                
                let v4_example_dir = v4_dir.join(&example.name);
                fs::create_dir_all(&v4_example_dir)?;
                copy_supporting_files(&example.path, &v4_example_dir)?;

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

                // Refresh YAML gallery if requested
                if args.refresh {
                    let yaml_example_dir = yaml_gallery.join(&example.name);
                    fs::create_dir_all(&yaml_example_dir)?;
                    copy_supporting_files(&example.path, &yaml_example_dir)?;
                    let _ = import::run(&v4_example_dir, &yaml_example_dir, AttributeStrategy::Inline);
                }
            }
            
            println!();
            println!("Upgraded: {} files ({} errors)", stats.upgraded, stats.upgrade_errors);
            generate_index(&output, &submodule_examples)?;
        }
    }

    // === BUILD: Build MSI packages ===
    if args.build {
        fs::create_dir_all(&msi_dir)?;
        
        // Discover examples based on source type
        let build_examples = discover_build_examples(
            &args.source_type,
            &yaml_gallery,
            &v4_dir,
            &configs_dir,
            filter.as_ref(),
        )?;
        
        if build_examples.is_empty() {
            println!("No buildable examples found");
        } else {
            let total = build_examples.len();
            let jobs = rayon::current_num_threads();
            println!();
            println!("=== Building MSI Packages ({} examples, {} parallel jobs) ===", total, jobs);
            
            // Counters for parallel builds
            let built = AtomicUsize::new(0);
            let skipped = AtomicUsize::new(0);
            let errors = AtomicUsize::new(0);
            let completed = AtomicUsize::new(0);
            let job_id = AtomicUsize::new(0);
            let output_lines = Mutex::new(Vec::new());
            
            // Build in parallel
            build_examples.par_iter().for_each(|example| {
                let my_job_id = job_id.fetch_add(1, Ordering::Relaxed);
                let result = build_example(example, &msi_dir, &args.wixer, args.verbose, my_job_id);
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                
                let line = match result {
                    BuildResult::Success => {
                        built.fetch_add(1, Ordering::Relaxed);
                        format!("[{}/{}] Building {}... ✓", done, total, example.name)
                    }
                    BuildResult::Skipped(reason) => {
                        skipped.fetch_add(1, Ordering::Relaxed);
                        format!("[{}/{}] Building {}... ⊘ ({})", done, total, example.name, reason)
                    }
                    BuildResult::Error(msg) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                        format!("[{}/{}] Building {}... ✗ ({})", done, total, example.name, msg)
                    }
                };
                
                // Print progress immediately
                println!("{}", line);
                output_lines.lock().unwrap().push((example.name.clone(), line));
            });
            
            stats.built = built.load(Ordering::Relaxed);
            stats.build_errors = errors.load(Ordering::Relaxed);
            
            println!();
            println!("MSI packages: {} built, {} skipped, {} failed", 
                stats.built, skipped.load(Ordering::Relaxed), stats.build_errors);
            println!("Output: {}", msi_dir.display());
        }

        // Run ICE validation if requested (parallel)
        if args.validate && stats.built > 0 {
            println!();
            
            let msi_files: Vec<_> = fs::read_dir(&msi_dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|e| e == "msi").unwrap_or(false))
                .collect();
            
            let total = msi_files.len();
            let jobs = rayon::current_num_threads();
            println!("=== Running ICE Validation ({} files, {} parallel jobs) ===", total, jobs);
            
            let validation_errors = AtomicUsize::new(0);
            let validation_warnings = AtomicUsize::new(0);
            let completed = AtomicUsize::new(0);
            
            msi_files.par_iter().for_each(|path| {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                
                let output = Command::new("wix")
                    .args(["msi", "validate", path.to_str().unwrap()])
                    .output();
                
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                
                let line = match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let combined = format!("{}{}", stdout, stderr);
                        
                        let errors: Vec<&str> = combined.lines()
                            .filter(|l| l.contains("error WIX"))
                            .collect();
                        let warnings: Vec<&str> = combined.lines()
                            .filter(|l| l.contains("warning WIX"))
                            .collect();
                        
                        if errors.is_empty() && warnings.is_empty() {
                            format!("[{}/{}] Validating {}... ✓", done, total, name)
                        } else if errors.is_empty() {
                            validation_warnings.fetch_add(warnings.len(), Ordering::Relaxed);
                            format!("[{}/{}] Validating {}... ⚠ ({} warnings)", done, total, name, warnings.len())
                        } else {
                            validation_errors.fetch_add(errors.len(), Ordering::Relaxed);
                            validation_warnings.fetch_add(warnings.len(), Ordering::Relaxed);
                            let error_details: String = errors.iter().map(|e| format!("\n  {}", e)).collect();
                            format!("[{}/{}] Validating {}... ✗ ({} errors, {} warnings){}", done, total, name, errors.len(), warnings.len(), error_details)
                        }
                    }
                    Err(e) => format!("[{}/{}] Validating {}... ✗ (wix not found: {})", done, total, name, e),
                };
                
                // Print progress immediately
                println!("{}", line);
            });
            
            let validation_errors = validation_errors.load(Ordering::Relaxed);
            let validation_warnings = validation_warnings.load(Ordering::Relaxed);
            
            println!();
            println!("Validation: {} errors, {} warnings", validation_errors, validation_warnings);
            
            if validation_errors > 0 {
                stats.build_errors += 1; // Count validation errors as build errors
            }
        }
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

struct Example {
    name: String,
    path: PathBuf,
    wxs_files: Vec<PathBuf>,
}

/// Example ready for building
struct BuildExample {
    name: String,
    source_dir: PathBuf,
    config: Option<ExampleConfig>,
    /// Files to build (.wxs.yaml or .wxs)
    build_files: Vec<PathBuf>,
}

enum BuildResult {
    Success,
    Skipped(String),
    Error(String),
}

/// Discover examples to build based on source type
fn discover_build_examples(
    source_type: &SourceType,
    yaml_gallery: &Path,
    v4_dir: &Path,
    configs_dir: &Path,
    filter: Option<&Vec<String>>,
) -> Result<Vec<BuildExample>> {
    let mut examples = Vec::new();
    
    // Discover from YAML gallery
    if *source_type == SourceType::Yaml || *source_type == SourceType::All {
        if yaml_gallery.exists() {
            for entry in fs::read_dir(yaml_gallery)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_dir() { continue; }
                
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                if name.starts_with('.') { continue; }
                
                // Apply filter
                if let Some(prefixes) = filter {
                    if !prefixes.iter().any(|p| name.starts_with(p)) { continue; }
                }
                
                // Load config
                let config_path = configs_dir.join(format!("{}.yaml", name));
                let config = ExampleConfig::load(&config_path).ok();
                
                // Find main YAML source files (.wxs.yaml only - .wxi.yaml are includes)
                let yaml_main_files: Vec<PathBuf> = WalkDir::new(&path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let fname = e.file_name().to_str().unwrap_or("");
                        fname.ends_with(".wxs.yaml")
                    })
                    .map(|e| e.path().to_path_buf())
                    .collect();
                
                // Find YAML template files (.yaml.in) - these are templates for <?include?>
                let yaml_template_files: Vec<PathBuf> = WalkDir::new(&path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let fname = e.file_name().to_str().unwrap_or("");
                        fname.ends_with(".yaml.in")
                    })
                    .map(|e| e.path().to_path_buf())
                    .collect();
                
                // Find XML files (for preprocessor examples)
                let xml_files: Vec<PathBuf> = WalkDir::new(&path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let fname = e.file_name().to_str().unwrap_or("");
                        fname.ends_with(".wxs") || fname.ends_with(".wxi")
                    })
                    .map(|e| e.path().to_path_buf())
                    .collect();
                
                // Determine build files:
                // Include all source files (.wxs.yaml, .wxs)
                // .yaml.in templates are handled by wixer build automatically
                let mut build_files = Vec::new();
                build_files.extend(yaml_main_files);
                build_files.extend(xml_files);
                // Don't include .yaml.in templates - wixer build exports them automatically
                
                if build_files.is_empty() {
                    continue;
                }
                
                examples.push(BuildExample {
                    name,
                    source_dir: path,
                    config,
                    build_files,
                });
            }
        }
    }
    
    // Discover from v4 (upgraded submodule) - only if not already found in YAML
    if *source_type == SourceType::Submodule || *source_type == SourceType::All {
        if v4_dir.exists() {
            // Collect names first to avoid borrow issues
            let yaml_names: std::collections::HashSet<String> = examples.iter().map(|e| e.name.clone()).collect();
            
            for entry in fs::read_dir(v4_dir)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_dir() { continue; }
                
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                if name.starts_with('.') { continue; }
                
                // Skip if already found in YAML gallery
                if yaml_names.contains(&name) { continue; }
                
                // Apply filter
                if let Some(prefixes) = filter {
                    if !prefixes.iter().any(|p| name.starts_with(p)) { continue; }
                }
                
                // Load config
                let config_path = configs_dir.join(format!("{}.yaml", name));
                let config = ExampleConfig::load(&config_path).ok();
                
                // Find WXS files
                let wxs_files: Vec<PathBuf> = WalkDir::new(&path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_str().map(|s| s.ends_with(".wxs")).unwrap_or(false))
                    .map(|e| e.path().to_path_buf())
                    .collect();
                
                if wxs_files.is_empty() { continue; }
                
                examples.push(BuildExample {
                    name,
                    source_dir: path,
                    config,
                    build_files: wxs_files,
                });
            }
        }
    }
    
    // Sort by name
    examples.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(examples)
}

/// Build a single example
fn build_example(example: &BuildExample, msi_dir: &Path, wixer_path: &Path, _verbose: bool, job_id: usize) -> BuildResult {
    // Check if buildable
    if let Some(ref config) = example.config {
        if !config.buildable {
            return BuildResult::Skipped(config.skip_reason.clone().unwrap_or_else(|| "not buildable".to_string()));
        }
    }
    
    let msi_path = msi_dir.join(format!("{}.msi", example.name));
    
    // Build command
    let mut cmd = Command::new(wixer_path);
    cmd.current_dir(&example.source_dir);
    // Set unique build ID for parallel builds
    cmd.env("WIXER_BUILD_ID", format!("{}", job_id));
    cmd.arg("build");
    
    // Add source files (only .wxs files, not .wxi) - use relative paths
    for file in &example.build_files {
        let fname = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.ends_with(".wxs.yaml") || fname.ends_with(".wxs") {
            // Convert to relative path from source_dir
            let rel_path = file.strip_prefix(&example.source_dir).unwrap_or(file);
            cmd.arg(rel_path);
        }
    }
    
    // Add config args (arch, defines, extensions)
    if let Some(ref config) = example.config {
        cmd.args(config.build_args());
    }
    
    cmd.args(["-o", msi_path.to_str().unwrap_or_default()]);
    
    // Debug: print command
    if _verbose {
        eprintln!("  Running: {:?}", cmd);
    }
    
    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                BuildResult::Success
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let combined = format!("{}\n{}", stderr, stdout);
                
                // Look for actual error lines
                let first_error = combined.lines()
                    .find(|l| l.contains("error WIX") || l.contains("error:"))
                    .unwrap_or("build failed");
                BuildResult::Error(first_error.chars().take(80).collect())
            }
        }
        Err(e) => BuildResult::Error(format!("exec failed: {}", e)),
    }
}

/// Find examples from submodule (v3 WiX files)
fn find_submodule_examples(source: &Path, filter: Option<&Vec<String>>) -> Result<Vec<Example>> {
    let mut examples = Vec::new();

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Skip non-example directories
        if name.starts_with('.') || name == "images" || name == ".github" {
            continue;
        }

        // Apply filter if specified
        if let Some(prefixes) = filter {
            if !prefixes.iter().any(|p| name.starts_with(p)) {
                continue;
            }
        }

        // Find all .wxs and .wxi files in this example
        let wxs_files: Vec<PathBuf> = WalkDir::new(&path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().is_some_and(|ext| ext == "wxs" || ext == "wxi")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        if !wxs_files.is_empty() {
            examples.push(Example {
                name,
                path,
                wxs_files,
            });
        }
    }

    // Sort by name
    examples.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(examples)
}

fn copy_supporting_files(src: &Path, dst: &Path) -> Result<()> {
    let supporting_extensions = ["md", "txt", "ps1", "cmd", "bat", "json"];

    for entry in WalkDir::new(src).max_depth(1) {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if supporting_extensions.contains(&ext) {
            let filename = path.file_name().unwrap();
            let dst_path = dst.join(filename);
            fs::copy(path, &dst_path)
                .with_context(|| format!("Failed to copy {}", path.display()))?;
        }
    }

    Ok(())
}

fn generate_index(output: &Path, examples: &[Example]) -> Result<()> {
    let mut content = String::new();

    content.push_str("# Wixer Example Gallery\n\n");
    content.push_str("This gallery contains WiX installer examples converted for use with wixer.\n\n");
    content.push_str("## Structure\n\n");
    content.push_str("- **wix-v4/** — WiX v4 XML files (upgraded from v3)\n");
    content.push_str("- **wixer-yaml/** — YAML equivalents for use with wixer\n\n");
    content.push_str("## Examples\n\n");
    content.push_str("| Example | Description |\n");
    content.push_str("|---------|-------------|\n");

    for example in examples {
        // Try to extract description from README
        let readme_path = example.path.join("README.md");
        let description = if readme_path.exists() {
            extract_first_paragraph(&readme_path).unwrap_or_default()
        } else {
            String::new()
        };

        content.push_str(&format!(
            "| [{}](wix-v4/{}) | {} |\n",
            example.name,
            example.name,
            description.chars().take(80).collect::<String>()
        ));
    }

    content.push_str("\n## Usage\n\n");
    content.push_str("```bash\n");
    content.push_str("# Build an example with wixer\n");
    content.push_str("wixer build wixer-yaml/00_HelloWorldInstaller/Product.wxs.yaml\n\n");
    content.push_str("# Or use the v4 XML directly with wix.exe\n");
    content.push_str("wix build wix-v4/00_HelloWorldInstaller/Product.wxs\n");
    content.push_str("```\n\n");
    content.push_str("## Rebuilding the Gallery\n\n");
    content.push_str("```bash\n");
    content.push_str("# From the wixer directory:\n");
    content.push_str("cargo run --bin wixer-gallery -- --clean\n");
    content.push_str("```\n\n");
    content.push_str("## Source\n\n");
    content.push_str("Examples are derived from [WiXInstallerExamples](https://github.com/kurtanr/WiXInstallerExamples) by kurtanr.\n");

    fs::write(output.join("README.md"), content)?;

    Ok(())
}

fn extract_first_paragraph(readme: &Path) -> Result<String> {
    let content = fs::read_to_string(readme)?;

    // Skip title and find first paragraph
    let mut in_paragraph = false;
    let mut paragraph = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip headers and empty lines until we find content
        if trimmed.starts_with('#') || trimmed.is_empty() {
            if in_paragraph {
                break;
            }
            continue;
        }

        // Skip badges
        if trimmed.starts_with("[![") || trimmed.starts_with("<br") {
            continue;
        }

        in_paragraph = true;
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }

    Ok(paragraph)
}

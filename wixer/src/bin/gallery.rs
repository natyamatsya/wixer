//! Gallery Builder - Builds the wixer example gallery from WiXInstallerExamples
//!
//! This tool processes the WiXInstallerExamples submodule and creates:
//! - v4/     WiX v4 XML files (upgraded from v3)
//! - yaml/   YAML equivalents of the v4 files
//!
//! Usage:
//!   wixer-gallery [--clean] [--source <path>] [--output <path>]

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use wixer::commands::{import, upgrade};
use wixer::common::AttributeStrategy;
use wixer::example_config::ExampleConfig;

#[derive(Parser)]
#[command(name = "wixer-gallery")]
#[command(about = "Build the wixer example gallery from WiXInstallerExamples")]
struct Args {
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

    /// Skip YAML conversion (only upgrade to v4)
    #[arg(long)]
    no_yaml: bool,

    /// Build MSI packages from the curated YAML gallery
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

    let source = if args.source.is_absolute() {
        args.source.clone()
    } else {
        std::env::current_dir()?.join(&args.source)
    };

    let output = if args.output.is_absolute() {
        args.output.clone()
    } else {
        std::env::current_dir()?.join(&args.output)
    };

    let yaml_gallery = if args.yaml_gallery.is_absolute() {
        args.yaml_gallery.clone()
    } else {
        std::env::current_dir()?.join(&args.yaml_gallery)
    };

    if !source.exists() {
        anyhow::bail!(
            "Source directory not found: {}\nRun from the wixer root or specify --source",
            source.display()
        );
    }

    // wix-v4/ is always regenerated from submodule (ephemeral)
    let v4_dir = output.join("wix-v4");
    // wixer-yaml/ is the curated gallery (tracked in git)
    // We only write to it with --refresh

    // Clean v4 output if requested (never clean curated yaml gallery automatically)
    if args.clean {
        if v4_dir.exists() {
            println!("Cleaning {}", v4_dir.display());
            fs::remove_dir_all(&v4_dir)?;
        }
    }

    fs::create_dir_all(&v4_dir)?;

    // Parse filter
    let filter: Option<Vec<String>> = args.only.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .collect()
    });

    // Find all example directories
    let examples = find_examples(&source, filter.as_ref())?;

    println!("Found {} examples to process", examples.len());
    println!();

    let mut stats = Stats::default();

    for example in &examples {
        println!("Processing: {}", example.name);

        // Create v4 output directory
        let v4_example_dir = v4_dir.join(&example.name);
        fs::create_dir_all(&v4_example_dir)?;

        // Copy non-WiX files (README, scripts, etc.) to v4 dir
        copy_supporting_files(&example.path, &v4_example_dir)?;

        // Process each .wxs file - upgrade v3 → v4
        for wxs_file in &example.wxs_files {
            let relative = wxs_file.strip_prefix(&example.path).unwrap_or(wxs_file);
            let v4_output = v4_example_dir.join(relative);

            // Create parent dirs
            if let Some(parent) = v4_output.parent() {
                fs::create_dir_all(parent)?;
            }

            // Upgrade v3 → v4
            match upgrade::run(wxs_file.clone(), Some(v4_output.clone())) {
                Ok(()) => {
                    stats.upgraded += 1;
                    if args.verbose {
                        println!("  ✓ Upgraded: {}", relative.display());
                    }
                }
                Err(e) => {
                    stats.upgrade_errors += 1;
                    eprintln!("  ✗ Failed to upgrade {}: {}", relative.display(), e);
                }
            }
        }

        // Refresh curated YAML gallery if requested
        if args.refresh {
            let yaml_example_dir = yaml_gallery.join(&example.name);
            fs::create_dir_all(&yaml_example_dir)?;
            copy_supporting_files(&example.path, &yaml_example_dir)?;
            
            match import::run(&v4_example_dir, &yaml_example_dir, AttributeStrategy::Inline) {
                Ok(()) => {
                    stats.converted += 1;
                    if args.verbose {
                        println!("  ✓ Refreshed YAML gallery");
                    }
                }
                Err(e) => {
                    stats.convert_errors += 1;
                    eprintln!("  ✗ Failed to convert to YAML: {}", e);
                }
            }
        }

        println!();
    }

    // Print summary
    println!("=== Gallery Upgrade Complete ===");
    println!("Examples processed: {}", examples.len());
    println!("Files upgraded:     {} ({} errors)", stats.upgraded, stats.upgrade_errors);
    if args.refresh {
        println!("YAML refreshed:     {} ({} errors)", stats.converted, stats.convert_errors);
    }
    println!();
    println!("Output:");
    println!("  v4:   {}", v4_dir.display());
    if args.refresh {
        println!("  yaml: {}", yaml_gallery.display());
    }

    // Generate index
    generate_index(&output, &examples)?;

    // Build MSI packages if requested (from curated YAML gallery)
    if args.build {
        println!();
        println!("=== Building MSI Packages (from curated gallery) ===");
        
        let msi_dir = output.join("msi");
        fs::create_dir_all(&msi_dir)?;
        
        // Resolve configs directory (relative paths are relative to CWD)
        let configs_dir = &args.configs;
        
        for example in &examples {
            // Load example configuration from configs directory
            let config_path = configs_dir.join(format!("{}.yaml", example.name));
            let example_config = if config_path.exists() {
                ExampleConfig::load(&config_path).ok()
            } else {
                None
            };
            
            // Check if example is marked as not buildable
            if let Some(ref config) = example_config {
                if !config.buildable {
                    print!("Building {}... ", example.name);
                    println!("⊘ (skipped: {})", config.skip_reason.as_deref().unwrap_or("not buildable"));
                    continue;
                }
            }
            
            // Build from curated wixer-yaml/ gallery
            let yaml_example_dir = yaml_gallery.join(&example.name);
            let v4_example_dir = v4_dir.join(&example.name);
            let product_yaml = find_product_yaml(&yaml_example_dir);
            
            if let Some(yaml_path) = product_yaml {
                let msi_path = msi_dir.join(format!("{}.msi", example.name));
                
                print!("Building {}... ", example.name);
                
                // Check if the "yaml" file is actually XML (copied due to preprocessor directives)
                let content = fs::read_to_string(&yaml_path).unwrap_or_default();
                let is_xml = content.trim_start().starts_with("<?xml") || content.trim_start().starts_with("<Wix");
                
                // Check if this is a multi-file project (has more than one .wxs file)
                let all_wxs = find_all_wxs(&v4_example_dir);
                let is_multi_file = all_wxs.len() > 1;
                
                let result = if is_xml || is_multi_file {
                    // Build from v4 XML directly using wix.exe
                    // For multi-file projects, we need to build all wxs files together
                    if all_wxs.is_empty() {
                        println!("✗ (no v4 wxs found)");
                        stats.build_errors += 1;
                        continue;
                    }
                    
                    // Build all wxs files together
                    let mut cmd = Command::new(&args.wixer);
                    cmd.current_dir(&v4_example_dir);
                    cmd.arg("build");
                    for wxs in &all_wxs {
                        cmd.arg(wxs.to_str().unwrap());
                    }
                    
                    // Add defines from config
                    if let Some(ref config) = example_config {
                        for (key, value) in &config.defines {
                            cmd.args(["-d", &format!("{}={}", key, value)]);
                        }
                    }
                    
                    cmd.args(["-o", msi_path.to_str().unwrap()]);
                    cmd.output()
                } else {
                    // Build from YAML using wixer (single file)
                    let mut cmd = Command::new(&args.wixer);
                    cmd.current_dir(&yaml_example_dir);
                    cmd.args(["build", yaml_path.to_str().unwrap()]);
                    
                    // Add defines from config
                    if let Some(ref config) = example_config {
                        for (key, value) in &config.defines {
                            cmd.args(["-d", &format!("{}={}", key, value)]);
                        }
                    }
                    
                    cmd.args(["-o", msi_path.to_str().unwrap()]);
                    cmd.output()
                };
                
                match result {
                    Ok(output) if output.status.success() => {
                        println!("✓");
                        stats.built += 1;
                    }
                    Ok(output) => {
                        println!("✗");
                        if args.verbose {
                            eprintln!("  {}", String::from_utf8_lossy(&output.stderr));
                        }
                        stats.build_errors += 1;
                    }
                    Err(e) => {
                        println!("✗ ({})", e);
                        stats.build_errors += 1;
                    }
                }
            }
        }
        
        println!();
        println!("MSI packages: {} built, {} failed", stats.built, stats.build_errors);
        println!("Output: {}", msi_dir.display());
    }

    let has_errors = stats.upgrade_errors > 0 || stats.convert_errors > 0 || stats.build_errors > 0;
    if has_errors {
        anyhow::bail!("Completed with errors");
    }

    Ok(())
}

fn find_product_yaml(dir: &Path) -> Option<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_str()
                .map(|s| s == "Product.wxs.yaml")
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
}

/// Find all .wxs files in a directory (for multi-file builds)
fn find_all_wxs(dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| s.ends_with(".wxs") && !s.ends_with(".wxi"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[derive(Default)]
struct Stats {
    upgraded: usize,
    upgrade_errors: usize,
    converted: usize,
    convert_errors: usize,
    built: usize,
    build_errors: usize,
}

struct Example {
    name: String,
    path: PathBuf,
    wxs_files: Vec<PathBuf>,
}

fn find_examples(source: &Path, filter: Option<&Vec<String>>) -> Result<Vec<Example>> {
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

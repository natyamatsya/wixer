use crate::common::AttributeStrategy;
use crate::config::WixerConfig;
use crate::constants::{
    EXT_MSI, EXT_WIXPDB, EXT_WSI, EXT_WSI_YAML, EXT_WXI, EXT_WXI_YAML, EXT_WXL, EXT_WXL_YAML, EXT_WXS,
    EXT_WXS_YAML, INTERMEDIATES_DIR, WIXER_DIR,
};
use crate::schema_mapping_v3v4::detect_required_extensions;
use crate::utils::{find_project_root, load_known_attributes};
use crate::yaml_to_wix;
use crate::commands::pdb;
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

/// Check if two executables are the same by comparing file size.
/// This detects infinite recursion when wixer is configured to call itself.
fn is_same_executable(path1: &Path, path2: &Path) -> bool {
    // First try canonical paths
    if let (Ok(c1), Ok(c2)) = (path1.canonicalize(), path2.canonicalize()) {
        if c1 == c2 {
            return true;
        }
    }
    
    // Compare file sizes (fast check)
    let meta1 = match std::fs::metadata(path1) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let meta2 = match std::fs::metadata(path2) {
        Ok(m) => m,
        Err(_) => return false,
    };
    
    if meta1.len() != meta2.len() {
        return false;
    }
    
    // If sizes match, compare file contents
    let content1 = match std::fs::read(path1) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let content2 = match std::fs::read(path2) {
        Ok(c) => c,
        Err(_) => return false,
    };
    
    content1 == content2
}

/// Resolve the wix executable path, checking WIX env var first (like Windows does)
fn resolve_wix_path(wix_exe: &str) -> PathBuf {
    let wix_path = PathBuf::from(wix_exe);
    
    // If absolute path, use it directly
    if wix_path.is_absolute() {
        return wix_path;
    }
    
    // Check WIX environment variable first (this is what CMake/CPack uses)
    if let Ok(wix_dir) = std::env::var("WIX") {
        let candidate = PathBuf::from(&wix_dir).join(format!("{}.exe", wix_exe));
        if candidate.exists() {
            return candidate;
        }
        let candidate = PathBuf::from(&wix_dir).join(wix_exe);
        if candidate.exists() {
            return candidate;
        }
    }
    
    // Fall back to PATH lookup
    which::which(wix_exe).unwrap_or(wix_path)
}


pub fn run(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        bail!("No arguments provided for build");
    }

    // Load config
    let config = WixerConfig::load();
    let mut build_args = args.clone();
    
    // Check for --profile arg manually since this is a raw vec
    let mut profile = None;
    if let Some(pos) = build_args.iter().position(|a| a == "--profile")
        && pos + 1 < build_args.len() {
            profile = Some(build_args[pos + 1].clone());
            // Remove from args passed to real wix
            build_args.remove(pos + 1);
            build_args.remove(pos);
        }

    // Check for --validate flag (wixer-specific, runs ICE validation after build)
    let run_validation = if let Some(pos) = build_args.iter().position(|a| a == "--validate") {
        build_args.remove(pos);
        true
    } else {
        false
    };

    let build_config = config.get_build_config(profile.as_deref());
    let wix_exe = build_config.real_wix_path.unwrap_or_else(|| "wix".to_string());

    // Detect infinite recursion: check if real_wix_path points to ourselves
    if let Ok(our_exe) = std::env::current_exe() {
        let resolved_wix = resolve_wix_path(&wix_exe);
        
        if is_same_executable(&our_exe, &resolved_wix) {
            bail!(
                "Infinite recursion detected: '{}' (resolved to '{}') points to wixer itself. \
                Please set real_wix_path in wixer.yaml to the actual WiX toolset executable, \
                or set the WIXER_CONFIG environment variable to point to a valid config file.",
                wix_exe,
                resolved_wix.display()
            );
        }
    }

    // Check if it's actually the 'build' command
    if build_args[0] != "build" {
        info!("Passthrough: {} {}", wix_exe, build_args.join(" "));
        let output = Command::new(&wix_exe)
            .args(&build_args)
            .output()
            .with_context(|| format!("Failed to execute wix ({wix_exe})"))?;
        
        // Log and print output
        if !output.stdout.is_empty() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            print!("{}", stdout);
            for line in stdout.lines() {
                info!("[wix stdout] {}", line);
            }
        }
        if !output.stderr.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprint!("{}", stderr);
            for line in stderr.lines() {
                info!("[wix stderr] {}", line);
            }
        }
        
        if !output.status.success() {
            let code = output.status.code().unwrap_or(1);
            if code > 0 {
                let error_name = crate::wix_exit_codes::get_error_name(code).unwrap_or("UnknownError");
                bail!("wix exited with error WIX{:04}: {} (exit code {})", code, error_name, code);
            } else {
                bail!("wix exited with code {}", code);
            }
        }
        return Ok(());
    }

    // Apply debug profile settings
    if build_config.verbose {
        info!("Debug profile active: Enabling verbose output");
        // Inject -v if not present
        if !build_args.iter().any(|a| a == "-v" || a == "-verbose") {
            build_args.push("-v".to_string());
        }
    }

    // It is 'build', let's intercept inputs
    // Use a unique intermediates directory per build to support parallel builds
    // Combine process ID (for cross-process uniqueness) with path hash (for within-process uniqueness)
    let unique_id = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        let cwd = std::env::current_dir().unwrap_or_default();
        cwd.hash(&mut hasher);
        // Include first non-flag arg as part of hash for uniqueness
        if let Some(first_input) = build_args.iter().find(|a| !a.starts_with('-')) {
            // Hash both the cwd-joined path and the filename for uniqueness
            let input_path = cwd.join(first_input);
            input_path.hash(&mut hasher);
            first_input.hash(&mut hasher);
        }
        format!("{}_{:x}", std::process::id(), hasher.finish())
    };
    
    let intermediates_dir = if let Ok(root) = find_project_root() {
        root.join(WIXER_DIR).join(INTERMEDIATES_DIR).join(&unique_id)
    } else {
        std::env::temp_dir().join("wixer_intermediates").join(&unique_id)
    };

    if !intermediates_dir.exists() {
        std::fs::create_dir_all(&intermediates_dir).expect("Failed to create intermediates dir");
    }

    // Load known attributes
    let known_attributes = load_known_attributes(None, true);

    let mut new_args = Vec::new();
    new_args.push("build".to_string());

    let mut output_path: Option<PathBuf> = None;
    let mut input_file: Option<PathBuf> = None;
    let mut converted_files: Vec<PathBuf> = Vec::new();
    let mut has_ext_arg = false;

    // Process build_args (which may have -v injected) instead of original args
    // build_args[0] is "build", we iterate from 1.

    let mut iter = build_args[1..].iter();
    while let Some(arg) = iter.next() {
        if arg == "-o" || arg == "--output" {
             new_args.push(arg.clone());
             if let Some(val) = iter.next() {
                 new_args.push(val.clone());
                 output_path = Some(PathBuf::from(val));
             }
             continue;
        }

        // Track if user already specified extensions
        if arg == "-ext" || arg.starts_with("-ext:") || arg.starts_with("--extension") {
            has_ext_arg = true;
        }

        if arg.starts_with("-") {
             new_args.push(arg.clone());
             continue;
        }

        // Potential input file
        let path = PathBuf::from(arg);
        if input_file.is_none() {
            input_file = Some(path.clone());
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Check for known extensions
        let (is_valid, target_ext) = if filename.ends_with(EXT_WXS_YAML) {
            (true, Some(EXT_WXS))
        } else if filename.ends_with(EXT_WXI_YAML) {
            (true, Some(EXT_WXI))
        } else if filename.ends_with(EXT_WXL_YAML) {
            (true, Some(EXT_WXL))
        } else if filename.ends_with(EXT_WSI_YAML) {
            (true, Some(EXT_WSI))
        } else {
            (false, None)
        };

        if is_valid && path.exists() {
            let target_extension = target_ext.unwrap();

            // Convert
            let filename = path.file_stem().unwrap().to_string_lossy();
            let output_filename = if filename.ends_with(&format!(".{}", target_extension)) {
                filename.to_string()
            } else {
                format!("{}.{}", filename, target_extension)
            };
            
            let output_file_path = intermediates_dir.join(&output_filename);

            yaml_to_wix::convert_file(
                &path,
                &output_file_path,
                AttributeStrategy::Inline,
                &known_attributes,
            )
            .with_context(|| format!("Failed to convert {}", path.display()))?;
            
            converted_files.push(output_file_path.clone());
            new_args.push(output_file_path.to_string_lossy().to_string());
            continue;
        }
        
        // Track non-YAML WiX files too for extension detection
        if path.exists() && (filename.ends_with(EXT_WXS) || filename.ends_with(EXT_WXI)) {
            converted_files.push(path.clone());
        }
        
        new_args.push(arg.clone());
    }
    
    // Export template files (.yaml.in suffix) referenced by <?include?> directives
    // e.g., if Product.wxs has <?include Foo.wxi?> and Foo.wxi.yaml.in exists, export it
    // Templates are exported to the intermediates directory to support parallel builds
    export_template_files(&converted_files, &intermediates_dir, &known_attributes)?;
    
    // Auto-detect required extensions if user didn't specify any
    if !has_ext_arg {
        let mut required_extensions: HashSet<&str> = HashSet::new();
        
        for file_path in &converted_files {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                for ext in detect_required_extensions(&content) {
                    required_extensions.insert(ext);
                }
            }
        }
        
        // Add detected extensions to args
        for ext in &required_extensions {
            info!("Auto-detected extension: {}", ext);
            new_args.push("-ext".to_string());
            new_args.push(ext.to_string());
        }
    }
    
    // Add intermediates directory to include search path for template exports
    new_args.push("-I".to_string());
    new_args.push(intermediates_dir.to_string_lossy().to_string());

    info!("Running: {} {}", wix_exe, new_args.join(" "));
    let output = Command::new(&wix_exe)
        .args(&new_args) // "build" is included in new_args[0]
        .output()
        .with_context(|| format!("Failed to execute wix ({wix_exe})"))?;

    // Log stdout and stderr from wix.exe
    if !output.stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            info!("[wix stdout] {}", line);
        }
    }
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            // WiX writes warnings and errors to stderr
            if line.contains("error") {
                warn!("[wix stderr] {}", line);
            } else {
                info!("[wix stderr] {}", line);
            }
        }
    }

    if !output.status.success() {
        let code = output.status.code().unwrap_or(1);
        if code > 0 {
            let error_name = crate::wix_exit_codes::get_error_name(code).unwrap_or("UnknownError");
            bail!("wix build failed with error WIX{:04}: {} (exit code {})", code, error_name, code);
        } else {
            bail!("wix build failed with exit code {}", code);
        }
    }

    // Determine output MSI path for post-build actions
    let msi_path = if let Some(ref out) = output_path {
        out.clone()
    } else if let Some(ref inp) = input_file {
        let file_stem = inp.file_stem().unwrap_or_default();
        PathBuf::from(file_stem).with_extension(EXT_MSI)
    } else {
        PathBuf::from(format!("output.{}", EXT_MSI))
    };

    // Post-build: ICE validation
    if run_validation {
        if msi_path.exists() {
            info!("Running ICE validation on {}...", msi_path.display());
            run_ice_validation(&msi_path)?;
        } else {
            warn!("MSI file not found for validation: {}", msi_path.display());
        }
    }

    // Post-build: PDB inspection
    if config.build.inspect_pdb {
        info!("Running post-build PDB inspection...");
        
        let pdb_path = msi_path.with_extension(EXT_WIXPDB);

        if pdb_path.exists() {
             for symbol in &build_config.inspect_symbols {
                 info!("Inspecting symbol: {}", symbol);
                 if let Err(e) = pdb::run(&pdb_path, Some(symbol.clone())) {
                     warn!("Failed to inspect PDB: {}", e);
                 }
             }
        } else {
             warn!("PDB file not found at expected location: {}", pdb_path.display());
        }
    }

    Ok(())
}

/// Run ICE validation on an MSI file using `wix msi validate`
fn run_ice_validation(msi_path: &Path) -> Result<()> {
    let output = Command::new("wix")
        .args(["msi", "validate", msi_path.to_str().unwrap_or_default()])
        .output()
        .context("Failed to run 'wix msi validate'. Is WiX Toolset installed?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Parse errors and warnings
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for line in combined.lines() {
        if line.contains("error WIX") {
            errors.push(line);
        } else if line.contains("warning WIX") {
            warnings.push(line);
        }
    }

    // Report results
    if errors.is_empty() && warnings.is_empty() {
        info!("ICE validation passed ✓");
    } else {
        if !warnings.is_empty() {
            warn!("ICE validation warnings ({}):", warnings.len());
            for w in &warnings {
                warn!("  {}", w);
            }
        }
        if !errors.is_empty() {
            for e in &errors {
                warn!("  {}", e);
            }
            bail!("ICE validation failed with {} error(s)", errors.len());
        }
    }

    Ok(())
}

/// Export YAML template files (.yaml.in suffix) to the intermediates directory
/// Scans XML files for <?include file.wxi?> directives and checks if file.wxi.yaml.in exists
fn export_template_files(files: &[PathBuf], intermediates_dir: &Path, known_attributes: &HashSet<String>) -> Result<()> {
    use regex::Regex;
    
    let include_re = Regex::new(r#"<\?include\s+([^?]+)\s*\?>"#).unwrap();
    
    for file_path in files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        
        let file_dir = file_path.parent().unwrap_or(Path::new("."));
        
        for cap in include_re.captures_iter(&content) {
            let include_file = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if include_file.is_empty() {
                continue;
            }
            
            let template_path = file_dir.join(format!("{}.yaml.in", include_file));
            
            // Check if YAML template file exists (e.g., Foo.wxi.yaml.in -> Foo.wxi)
            if template_path.exists() {
                // Export to intermediates directory for parallel build safety
                let output_path = intermediates_dir.join(include_file);
                info!("Exporting template: {} -> {}", template_path.display(), output_path.display());
                
                // Convert YAML template to XML
                yaml_to_wix::convert_file(
                    &template_path,
                    &output_path,
                    crate::common::AttributeStrategy::Inline,
                    known_attributes,
                ).with_context(|| format!("Failed to export template {}", template_path.display()))?;
            }
        }
    }
    
    Ok(())
}

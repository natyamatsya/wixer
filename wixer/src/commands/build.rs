use crate::common::AttributeStrategy;
use crate::config::WixerConfig;
use crate::constants::{
    EXT_WIXPDB, EXT_WSI, EXT_WSI_YAML, EXT_WXI, EXT_WXI_YAML, EXT_WXL, EXT_WXL_YAML, EXT_WXS,
    EXT_WXS_YAML, INTERMEDIATES_DIR, WIXER_DIR,
};
use crate::utils::{find_project_root, load_known_attributes};
use crate::yaml_to_wix;
use crate::commands::pdb;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn};


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

    let build_config = config.get_build_config(profile.as_deref());
    let wix_exe = build_config.real_wix_path.unwrap_or_else(|| "wix".to_string());

    // Check if it's actually the 'build' command
    if build_args[0] != "build" {
        let status = Command::new(&wix_exe)
            .args(build_args)
            .status()
            .with_context(|| format!("Failed to execute wix ({wix_exe})"))?;
        if !status.success() {
            let code = status.code().unwrap_or(1);
            bail!("wix exited with code {code}");
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
    let intermediates_dir = if let Ok(root) = find_project_root() {
        root.join(WIXER_DIR).join(INTERMEDIATES_DIR)
    } else {
         std::env::temp_dir().join("wixer_intermediates")
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

    // Skip the first arg "build" when iterating for processing, but we need it in new_args?
    // args[0] is "build".
    // We iterate from 1.

    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        if arg == "-o" || arg == "--output" {
             new_args.push(arg.clone());
             if let Some(val) = iter.next() {
                 new_args.push(val.clone());
                 output_path = Some(PathBuf::from(val));
             }
             continue;
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
            
            let output_file_path = intermediates_dir.join(output_filename);

            yaml_to_wix::convert_file(
                &path,
                &output_file_path,
                AttributeStrategy::Inline,
                &known_attributes,
            )
            .with_context(|| format!("Failed to convert {}", path.display()))?;
            new_args.push(output_file_path.to_string_lossy().to_string());
            continue;
        }
        new_args.push(arg.clone());
    }

    info!("Running: {} {}", wix_exe, new_args.join(" "));
    let status = Command::new(&wix_exe)
        .args(&new_args) // "build" is included in new_args[0]
        .status()
        .with_context(|| format!("Failed to execute wix ({wix_exe})"))?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        bail!("wix build failed with exit code {code}");
    }

    // Post-build actions
    if config.build.inspect_pdb {
        info!("Running post-build PDB inspection...");
        
        let pdb_path = if let Some(out) = output_path {
            out.with_extension(EXT_WIXPDB)
        } else if let Some(inp) = input_file {
            // Default output is input filename + .msi? No, wix defaults to input filename basis
            // If input is CloseApp.wxs, output is CloseApp.msi/wixpdb
            let file_stem = inp.file_stem().unwrap_or_default();
            // Assuming current directory for output if not specified? 
            // Wix 4 usually outputs to current dir unless -o is specified.
            PathBuf::from(file_stem).with_extension(EXT_WIXPDB)
        } else if let Some(input) = input_file.as_ref() {
             input.with_extension(EXT_WIXPDB)
        } else {
             PathBuf::from(format!("output.{}", EXT_WIXPDB))
        };

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

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use wixer::gallery::options::{GalleryOptions, SourceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SourceTypeArg {
    Yaml,
    Submodule,
    All,
}

impl From<SourceTypeArg> for SourceType {
    fn from(value: SourceTypeArg) -> Self {
        match value {
            SourceTypeArg::Yaml => SourceType::Yaml,
            SourceTypeArg::Submodule => SourceType::Submodule,
            SourceTypeArg::All => SourceType::All,
        }
    }
}

#[derive(Parser)]
#[command(name = "wixer-gallery")]
#[command(about = "Build the wixer example gallery")]
struct Args {
    #[arg(long, default_value = "all")]
    source_type: SourceTypeArg,

    #[arg(short, long, default_value = "examples/WiXInstallerExamples")]
    source: PathBuf,

    #[arg(short, long, default_value = "examples/gallery")]
    output: PathBuf,

    #[arg(short, long, default_value = "examples/configs")]
    configs: PathBuf,

    #[arg(long, default_value = "examples/wixer-yaml")]
    yaml_gallery: PathBuf,

    #[arg(long)]
    clean: bool,

    #[arg(long)]
    only: Option<String>,

    #[arg(long)]
    upgrade: bool,

    #[arg(short, long)]
    build: bool,

    #[arg(long)]
    refresh: bool,

    #[arg(long, default_value = "wixer")]
    wixer: PathBuf,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long)]
    validate: bool,

    /// Generate MSIX manifest outputs via `wixer migrate --target msix`
    #[arg(long)]
    msix: bool,

    #[arg(short = 'j', long, default_value = "0")]
    jobs: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    if args.jobs > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.jobs)
            .build_global()
            .ok();
    }

    let only_prefixes: Option<Vec<String>> = args
        .only
        .as_ref()
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect());

    let options = GalleryOptions {
        source_type: args.source_type.into(),
        source: args.source,
        output: args.output,
        configs: args.configs,
        yaml_gallery: args.yaml_gallery,
        clean: args.clean,
        only_prefixes,
        upgrade: args.upgrade,
        build: args.build,
        refresh: args.refresh,
        wixer: args.wixer,
        verbose: args.verbose,
        validate: args.validate,
        msix: args.msix,
    };

    wixer::gallery::run_with_options(options)
}

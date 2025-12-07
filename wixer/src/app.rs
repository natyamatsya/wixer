use std::collections::{BTreeSet, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use anyhow::{anyhow, bail, ensure, Context, Result};
use clap::{Parser, Subcommand};
use tokio::task::spawn_blocking;
use tracing::{error, info};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, util::SubscriberInitExt};

use crate::commands;
use crate::common::AttributeStrategy;
use crate::config::WixerConfig;
use crate::constants::{DOWNLOADED_SCHEMAS_DIR, GENERATED_SCHEMAS_DIR, KNOWN_ATTRIBUTES_FILE};
use crate::utils::load_known_attributes;
use crate::{validate, wix_to_yaml, xsd_to_json, yaml_to_wix};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

pub async fn run() -> Result<()> {
    let config = WixerConfig::load();
    setup_logging(&config)?;

    if handle_impersonation().await? {
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::XsdToJson {
            input,
            output,
            download_wix,
            schema_dir,
            output_dir,
            attribute_strategy,
            no_descriptions,
            strict,
            root_element,
        } => {
            handle_xsd_to_json(XsdToJsonArgs {
                input,
                output,
                download_wix,
                schema_dir,
                output_dir,
                attribute_strategy,
                no_descriptions,
                strict,
                root_element,
            })
            .await?;
        }
        Commands::YamlToWix {
            input,
            output,
            input_dir,
            output_dir,
            attribute_strategy,
            attributes_file,
        } => {
            run_blocking(move || {
                handle_yaml_to_wix(
                    input,
                    output,
                    input_dir,
                    output_dir,
                    attribute_strategy,
                    attributes_file,
                )
            })
            .await?;
        }
        Commands::WixToYaml {
            input,
            output,
            attribute_strategy,
        } => {
            run_blocking(move || handle_wix_to_yaml(input, output, attribute_strategy)).await?;
        }
        Commands::Validate {
            schema,
            yaml,
            schema_dir,
            yaml_dir,
        } => {
            run_blocking(move || handle_validate(schema, yaml, schema_dir, yaml_dir)).await?;
        }
        Commands::Import {
            input_dir,
            output_dir,
            attribute_strategy,
        } => {
            run_blocking(move || commands::import::run(&input_dir, &output_dir, attribute_strategy))
                .await?;
        }
        Commands::Export {
            input_dir,
            output_dir,
            attribute_strategy,
        } => {
            run_blocking(move || commands::export::run(&input_dir, &output_dir, attribute_strategy))
                .await?;
        }
        Commands::Describe { term } => {
            run_blocking(move || commands::describe::run(term)).await?;
        }
        Commands::Build(args) => {
            run_blocking(move || commands::build::run(args)).await?;
        }
        Commands::Doc { output_dir, format } => {
            run_blocking(move || commands::doc::run(&output_dir, &format)).await?;
        }
        Commands::MapSchemas => {
            run_blocking(commands::map_schemas::run).await?;
        }
        Commands::Migrate {
            input,
            output_dir,
            target,
        } => {
            run_blocking(move || commands::migrate::run(input, output_dir, target)).await?;
        }
        Commands::Upgrade { input, output } => {
            run_blocking(move || commands::upgrade::run(input, output)).await?;
        }
        Commands::InspectPdb { path, symbol } => {
            run_blocking(move || commands::pdb::run(&path, symbol)).await?;
        }
        Commands::Sequence { path, table } => {
            run_blocking(move || commands::sequence::run(&path, table)).await?;
        }
        Commands::Lsp => {
            commands::lsp::run().await?;
        }
        Commands::Init { fresh } => {
            commands::init::run(fresh).await?;
        }
    }

    Ok(())
}

fn setup_logging(config: &WixerConfig) -> Result<()> {
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(LevelFilter::INFO);

    if let Some(log_file) = &config.logging.file {
        match std::fs::File::create(log_file) {
            Ok(file) => {
                let file_layer = fmt::layer()
                    .with_writer(file)
                    .with_ansi(false)
                    .with_filter(LevelFilter::DEBUG);

                tracing_subscriber::registry()
                    .with(stderr_layer)
                    .with(file_layer)
                    .try_init()
                    .map_err(|e| anyhow!("Failed to initialize logging: {e}"))?;
            }
            Err(err) => {
                eprintln!("Failed to create log file {}: {}", log_file, err);

                tracing_subscriber::registry()
                    .with(fmt::layer().with_writer(std::io::stderr).with_filter(LevelFilter::INFO))
                    .try_init()
                    .map_err(|e| anyhow!("Failed to initialize logging: {e}"))?;
            }
        }
    } else {
        tracing_subscriber::registry()
            .with(stderr_layer)
            .try_init()
            .map_err(|e| anyhow!("Failed to initialize logging: {e}"))?;
    }

    Ok(())
}

async fn handle_impersonation() -> Result<bool> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(name) = exe.file_stem()
        && name.to_string_lossy().eq_ignore_ascii_case("wix")
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if !args.is_empty() {
            let cmd = &args[0];
            let wixer_cmds = [
                "xsd-to-json",
                "yaml-to-wix",
                "wix-to-yaml",
                "validate",
                "init",
                "import",
                "export",
                "describe",
                "doc",
                "lsp",
                "map-schemas",
                "migrate",
                "upgrade",
                "build",
            ];

            if !wixer_cmds.contains(&cmd.as_str()) {
                run_blocking(move || commands::build::run(args)).await?;
                return Ok(true);
            }
        }
    }

    Ok(false)
}

struct XsdToJsonArgs {
    input: Option<String>,
    output: Option<String>,
    download_wix: bool,
    schema_dir: PathBuf,
    output_dir: PathBuf,
    attribute_strategy: AttributeStrategy,
    no_descriptions: bool,
    strict: bool,
    root_element: Option<String>,
}

async fn handle_xsd_to_json(args: XsdToJsonArgs) -> Result<()> {
    let XsdToJsonArgs {
        input,
        output,
        download_wix,
        schema_dir,
        output_dir,
        attribute_strategy,
        no_descriptions,
        strict,
        root_element,
    } = args;
    ensure!(
        download_wix || input.is_some(),
        "Either --input or --download-wix is required"
    );

    let mut all_attributes = HashSet::new();

    if download_wix {
        let files = xsd_to_json::download_wix_schemas(&schema_dir, false)
            .await
            .context("Error downloading WiX schemas")?;

        for file in files {
            match xsd_to_json::convert_xsd(
                file.to_str().unwrap_or_default(),
                None,
                &output_dir,
                attribute_strategy,
                !no_descriptions,
                strict,
                root_element.as_deref(),
            ) {
                Ok(attrs) => all_attributes.extend(attrs),
                Err(e) => error!(file = %file.display(), "Error converting: {}", &e),
            }
        }
    }

    if let Some(input_path) = input {
        match xsd_to_json::convert_xsd(
            &input_path,
            output.as_deref(),
            &output_dir,
            attribute_strategy,
            !no_descriptions,
            strict,
            root_element.as_deref(),
        ) {
            Ok(attrs) => all_attributes.extend(attrs),
            Err(e) => bail!("Error converting XSD: {e}"),
        }
    }

    if !all_attributes.is_empty() {
        let sorted_attributes: BTreeSet<_> = all_attributes.into_iter().collect();
        let attr_file = output_dir.join(KNOWN_ATTRIBUTES_FILE);
        let file = File::create(&attr_file)
            .with_context(|| format!("Failed to create {}", attr_file.display()))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &sorted_attributes)
            .context("Failed to write attributes")?;
        info!("Generated attribute list: {}", attr_file.display());
    }

    Ok(())
}

fn handle_yaml_to_wix(
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    input_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    attribute_strategy: AttributeStrategy,
    attributes_file: Option<PathBuf>,
) -> Result<()> {
    let known_attributes = load_known_attributes(
        attributes_file.as_deref(),
        attribute_strategy == AttributeStrategy::Inline,
    );

    if let (Some(in_dir), Some(out_dir)) = (input_dir, output_dir) {
        yaml_to_wix::convert_directory(&in_dir, &out_dir, attribute_strategy, &known_attributes)?;
        return Ok(());
    }

    if let (Some(input_path), Some(output_path)) = (input, output) {
        yaml_to_wix::convert_file(&input_path, &output_path, attribute_strategy, &known_attributes)?;
        return Ok(());
    }

    bail!(
        "Either provide input and output files, or use --input-dir and --output-dir"
    );
}

fn handle_wix_to_yaml(
    input: PathBuf,
    output: PathBuf,
    attribute_strategy: AttributeStrategy,
) -> Result<()> {
    let known_attributes = HashSet::new();
    wix_to_yaml::convert_file(&input, &output, attribute_strategy, &known_attributes)
        .context("Error converting WiX to YAML")?;
    Ok(())
}

fn handle_validate(
    schema: Option<PathBuf>,
    yaml: Option<PathBuf>,
    schema_dir: Option<PathBuf>,
    yaml_dir: Option<PathBuf>,
) -> Result<()> {
    if let (Some(s_dir), Some(y_dir)) = (schema_dir, yaml_dir) {
        return validate::validate_directory(&s_dir, &y_dir)
            .context("Error validating directory");
    }

    if let (Some(schema_path), Some(yaml_path)) = (schema, yaml) {
        return validate::validate_file(&schema_path, &yaml_path)
            .context("Validation failed");
    }

    bail!("Either provide schema and yaml files, or use --schema-dir and --yaml-dir");
}

async fn run_blocking<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| anyhow!("Blocking task failed: {e}"))?
}

#[derive(Subcommand)]
enum Commands {
    /// Convert XSD schemas to JSON Schema
    XsdToJson {
        /// Input XSD file path or URL
        #[arg(short, long)]
        input: Option<String>,

        /// Output JSON Schema file
        #[arg(short, long)]
        output: Option<String>,

        /// Download all WiX v4 schemas from GitHub
        #[arg(long)]
        download_wix: bool,

        /// Directory for downloaded schemas
        #[arg(long, default_value = DOWNLOADED_SCHEMAS_DIR)]
        schema_dir: PathBuf,

        /// Directory for generated JSON schemas
        #[arg(long, default_value = GENERATED_SCHEMAS_DIR)]
        output_dir: PathBuf,

        /// How to handle XML attributes
        #[arg(long, value_enum, default_value_t = AttributeStrategy::Inline)]
        attribute_strategy: AttributeStrategy,

        /// Exclude descriptions from schemas
        #[arg(long)]
        no_descriptions: bool,

        /// Enable strict mode (enforce required fields)
        #[arg(long)]
        strict: bool,

        /// Root element name for schema
        #[arg(long)]
        root_element: Option<String>,
    },
    /// Convert YAML to WiX XML
    YamlToWix {
        /// Input YAML file
        #[arg(index = 1)]
        input: Option<PathBuf>,

        /// Output WiX XML file
        #[arg(index = 2)]
        output: Option<PathBuf>,

        /// Input directory containing YAML files
        #[arg(long)]
        input_dir: Option<PathBuf>,

        /// Output directory for WiX XML files
        #[arg(long)]
        output_dir: Option<PathBuf>,

        /// Attribute handling strategy (must match XSD conversion)
        #[arg(long, value_enum, default_value_t = AttributeStrategy::Inline)]
        attribute_strategy: AttributeStrategy,

        /// Path to known_attributes.json (generated by xsd-to-json)
        #[arg(long)]
        attributes_file: Option<PathBuf>,
    },
    /// Convert WiX XML to YAML
    WixToYaml {
        /// Input WiX XML file
        #[arg(index = 1)]
        input: PathBuf,

        /// Output YAML file
        #[arg(index = 2)]
        output: PathBuf,

        /// Attribute handling strategy
        #[arg(long, value_enum, default_value_t = AttributeStrategy::Inline)]
        attribute_strategy: AttributeStrategy,
    },
    /// Validate YAML against JSON Schema
    Validate {
        /// JSON Schema file
        #[arg(index = 1)]
        schema: Option<PathBuf>,

        /// YAML file to validate
        #[arg(index = 2)]
        yaml: Option<PathBuf>,

        /// Directory containing JSON schemas
        #[arg(long)]
        schema_dir: Option<PathBuf>,

        /// Directory containing YAML files to validate
        #[arg(long)]
        yaml_dir: Option<PathBuf>,
    },
    /// Initialize wixer by downloading XSD schemas to a local cache
    Init {
        /// Delete existing cache and re-download schemas
        #[arg(long)]
        fresh: bool,
    },
    /// Recursively import WiX files from a directory
    Import {
        /// Input directory containing .wxs or .wxs.in files
        #[arg(index = 1)]
        input_dir: PathBuf,

        /// Output directory for .yaml files
        #[arg(index = 2)]
        output_dir: PathBuf,

        /// Attribute handling strategy
        #[arg(long, value_enum, default_value_t = AttributeStrategy::Inline)]
        attribute_strategy: AttributeStrategy,
    },
    /// Recursively export YAML files to WiX XML
    Export {
        /// Input directory containing .wys, .wyi, .wyl files
        #[arg(index = 1)]
        input_dir: PathBuf,

        /// Output directory for WiX files
        #[arg(index = 2)]
        output_dir: PathBuf,

        /// Attribute handling strategy
        #[arg(long, value_enum, default_value_t = AttributeStrategy::Inline)]
        attribute_strategy: AttributeStrategy,
    },
    /// Describe WiX properties, variables, and concepts
    Describe {
        /// Search term (e.g. "ProductName", "INSTALLFOLDER")
        #[arg(index = 1)]
        term: String,
    },
    /// Wrapper around 'wix build' that transparently converts YAML files
    #[command(external_subcommand)]
    Build(Vec<String>),
    /// Generate documentation from XSD schemas
    Doc {
        /// Output directory for documentation
        #[arg(long, default_value = "wix_docs")]
        output_dir: PathBuf,

        /// Format to generate (markdown, html, or all)
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Start the WiX Language Server
    Lsp,
    /// Map WiX schemas to MSIX schemas
    MapSchemas,
    /// Migrate WiX project to another format
    Migrate {
        /// Input WiX file
        #[arg(index = 1)]
        input: PathBuf,

        /// Output directory
        #[arg(index = 2)]
        output_dir: PathBuf,

        /// Migration target
        #[arg(long, value_enum)]
        target: commands::migrate::MigrationTarget,
    },
    /// Upgrade WiX v3 files to v4/v6
    Upgrade {
        /// Input file
        #[arg(index = 1)]
        input: PathBuf,

        /// Output file
        #[arg(index = 2)]
        output: Option<PathBuf>,
    },
    /// Inspect WiX PDB file
    InspectPdb {
        /// Path to .wixpdb file
        #[arg(index = 1)]
        path: PathBuf,

        /// Symbol ID to lookup
        #[arg(long)]
        symbol: Option<String>,
    },
    /// Visualize execution sequence
    Sequence {
        /// Path to .wixpdb file
        #[arg(index = 1)]
        path: PathBuf,

        /// Filter by table name (e.g. InstallExecuteSequence)
        #[arg(long)]
        table: Option<String>,
    },
}

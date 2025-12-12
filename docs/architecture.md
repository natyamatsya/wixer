# Architecture

## Overview

Wixer is a Rust CLI tool for authoring WiX installers using YAML instead of XML.

There are two core ideas:

1. **Conversion**: YAML sources (`.wxs.yaml`, `.wxi.yaml`, `.wxl.yaml`) are converted to WiX XML (`.wxs`, `.wxi`, `.wxl`).
2. **Wrapping/impersonation**: Wixer can transparently intercept calls to the WiX toolset (`wix.exe`) and run the conversion automatically.

## Entry points

- `wixer/src/main.rs`
  - Async entrypoint; calls `app::run()`.

- `wixer/src/app.rs`
  - CLI definition (`clap`) and command dispatch.
  - Implements *impersonation mode*: if the executable name is `wix.exe`, it forwards the command to `commands::build::run(...)`.

## Major modules

- `src/commands/`
  - Subcommand implementations.
  - Notable commands:
    - `build`: wraps `wix build`, converts YAML inputs into `.wixer/intermediates/...` and calls the real WiX.
    - `export` / `import`: batch conversion.
    - `validate`: validates YAML against JSON Schema.
    - `describe`: documentation/knowledge-base lookup.
    - `lsp`: language server.
    - `doc`: generates schema documentation.
    - `upgrade`: WiX v3 → v4/v6 upgrade pipeline.

- `src/yaml_to_wix.rs`
  - YAML → WiX XML conversion.

- `src/wix_to_yaml.rs`
  - WiX XML → YAML conversion.

- `src/upgrade/`
  - Upgrade pipeline from WiX v3 syntax to modern WiX.

- `src/simulation/`
  - MSI execution simulation.

- `src/analysis/`
  - Static analysis (pre-build checks).

## Project-local state

Wixer may create a project-local directory:

- `.wixer/intermediates/`
  - Generated XML outputs used when wrapping `wix build`.
  - The build command uses a per-build unique id (`WIXER_BUILD_ID` or a hash) to support parallel builds.

## Configuration

- `wixer.yaml` is loaded by walking up from the current directory, then falling back to a global config directory.

## Embedded assets

Wixer embeds schemas and metadata at compile time. See `embedded-assets.md`.

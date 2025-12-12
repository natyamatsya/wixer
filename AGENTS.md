# AI Agent Onboarding

## Project Overview

**Wixer** is a Rust CLI tool that enables authoring WiX (Windows Installer XML) projects using YAML instead of XML. It wraps the official `wix.exe` toolset, transparently converting YAML sources to WiX XML at build time.

## Repository Structure

```text
wixer/
├── wixer/                  # Main Rust crate (CLI binary)
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── app.rs          # CLI definition (clap) and command dispatch
│   │   ├── lib.rs          # Library exports
│   │   ├── commands/       # Subcommand implementations
│   │   ├── upgrade/        # WiX v3→v4 upgrade pipeline
│   │   ├── simulation/     # MSI execution simulation
│   │   ├── analysis/       # Code analysis (orphan detection, PDB validation)
│   │   ├── yaml_to_wix.rs  # YAML → WiX XML conversion
│   │   ├── wix_to_yaml.rs  # WiX XML → YAML conversion
│   │   ├── xsd_to_json.rs  # XSD → JSON Schema conversion
│   │   ├── config.rs       # Configuration loading (wixer.yaml)
│   │   ├── dirs.rs         # Platform-specific directories
│   │   ├── utils.rs        # Utility functions (project root, attributes)
│   │   ├── embedded_assets.rs # Compile-time embedded assets (schemas, extensions)
│   │   ├── wix_exit_codes.rs # WiX error ID → name mapping
│   │   ├── validate.rs     # YAML validation against JSON Schema
│   │   ├── project.rs      # Project file handling
│   │   ├── example_config.rs # Gallery example configuration schema
│   │   ├── schema_mapping_v3v4.rs # WiX v3→v4 upgrade mappings
│   │   └── constants.rs    # File extensions, namespaces
│   ├── assets/
│   │   └── assets.zip      # Embedded assets (store-only zip for zero-copy access)
│   ├── src/bin/
│   │   └── gallery.rs      # Gallery test runner binary
│   ├── examples/           # WiX example gallery
│   │   ├── WiXInstallerExamples/ # Submodule with v3 examples
│   │   ├── wixer-yaml/     # Curated YAML examples
│   │   ├── wix/            # Generated v4 XML outputs
│   │   └── configs/        # Example build configurations
│   └── Cargo.toml
├── wixer-vscode/           # VS Code extension (TypeScript)
│   ├── src/extension.ts    # Extension entry point (LSP client)
│   ├── syntaxes/           # TextMate grammar for syntax highlighting
│   └── package.json        # Extension manifest
├── wix/                    # Vendored WiX Toolset source (reference)
├── cmake/                  # CMake integration modules
│   ├── WixerImpersonate.cmake  # Transparent YAML support for CMake WIX generator
│   ├── MakeWixerMSIX.cmake.in  # MSIX packaging via wixer migrate
│   └── MakeAppX.cmake.in       # MSIX packaging via MakeAppX
├── CMakeLists.txt          # CMake build for MSIX packaging
├── CMakePresets.json       # CMake presets for MSI/MSIX generation
├── Makefile.toml           # cargo-make tasks
├── wixer.wxs.yaml          # Example/dogfood WiX project in YAML
└── Cargo.toml              # Workspace root
```

## Key Commands

| Command | Purpose |
|---------|---------|
| `wixer build <file.wxs.yaml>` | Convert YAML and invoke `wix build` |
| `wixer init [--fresh]` | Download WiX schemas to global cache |
| `wixer import <dir> <out>` | Batch convert `.wxs` → `.wxs.yaml` |
| `wixer export <dir> <out>` | Batch convert `.wxs.yaml` → `.wxs` |
| `wixer describe <term>` | Lookup WiX properties/variables/custom actions |
| `wixer lsp` | Start Language Server Protocol server |
| `wixer migrate --target msix` | Convert WiX project to MSIX manifest |
| `wixer upgrade <file>` | Upgrade WiX v3 syntax to v4/v6 |
| `wixer doc` | Generate documentation from XSD schemas |
| `wixer inspect-pdb <file>` | Inspect WiX PDB file contents |
| `wixer sequence <file>` | Visualize installer execution sequence |
| `wixer simulate <file>` | Simulate MSI execution for different install modes |

## File Extensions

| YAML Source | WiX Target | Purpose |
|-------------|------------|---------|
| `.wxs.yaml` | `.wxs` | Main installer source |
| `.wxi.yaml` | `.wxi` | Include files |
| `.wxl.yaml` | `.wxl` | Localization |

## Configuration

Wixer loads `wixer.yaml` from:

1. Walking up from CWD (project-local)
2. Global config directory (`%APPDATA%\natyamatsya\wixer\config` on Windows)

```yaml
build:
  verbose: false
  inspect_pdb: false
  real_wix_path: "C:\\path\\to\\wix.exe"  # Optional override
profiles:
  debug:
    verbose: true
    inspect_pdb: true
```

## Cache & Schemas

Global cache at `%LOCALAPPDATA%\natyamatsya\wixer\cache` (Windows) stores:

- Downloaded WiX XSD schemas (or extracted from embedded assets)
- Generated JSON schemas for validation
- `describe.json` knowledge base
- `LICENSE.txt` with attribution for WiX Toolset and MSIX schemas

Run `wixer init` to populate the cache. In CI environments without network access, wixer automatically extracts embedded assets as a fallback.

## Embedded Assets

Wixer embeds all required assets at compile time in `assets/assets.zip` (store-only, no compression for zero-copy memory access):

- `wix-v4-xsd-schemas/` — WiX v4 XSD schemas (14 files)
- `ms-msix-xsd-schemas/` — Microsoft MSIX XSD schemas (54 files across year subdirs)
- `wix-v4-extensions-json/` — WiX v4 extension metadata JSON (custom actions, sequences, properties)
- `wixer-knowledge-base.json` — MSI property/variable documentation

The `assets.zip` is **generated by `build.rs`** at compile time from the source asset directories. It is not checked into git. The build script:
- Creates a store-only (uncompressed) zip for zero-copy memory access
- Only rebuilds when source files change (tracks modification times)
- Requires `assets/wix-v4-extensions-json/` and `assets/wixer-knowledge-base.json` (tracked in git)
- Requires `assets/wix-v4-xsd-schemas/` and `assets/ms-msix-xsd-schemas/` (downloaded by `wixer init`)

The `embedded_assets` module provides:
- `extract_wix_schemas()` / `extract_msix_schemas()` — Extract to cache directory
- `get_extension(name)` / `get_knowledge_base()` — Direct access without extraction
- `has_embedded_assets()` — Check availability
- `get_versions()` — Get the `versions.json` manifest with build info and content hashes

The `versions.json` manifest contains:
- `build_time` — ISO-8601 timestamp of when assets.zip was built
- `git_commit` — Git commit hash for git-tracked assets
- `assets` — Per-asset metadata including source URLs and content hashes

## Build System

- **Rust**: Edition 2024, workspace with single `wixer` member
- **CI**: GitHub Actions (`.github/workflows/ci.yml`) runs `cargo fmt`, `clippy`, `nextest`, coverage
- **Packaging**: CMake + CPack generates MSIX via `MakeAppX`

The `CMakeLists.txt` and `Makefile.toml` configurations serve as **dogfooding examples**—Wixer builds its own Windows installers (MSI via WiX, MSIX via MakeAppX) using itself.

### CMake Integration

Wixer provides CMake modules for transparent YAML support:

- **`WixerImpersonate.cmake`**: Intercepts CMake's WIX generator to convert YAML sources automatically
- **`wixer_add_yaml_sources()`**: Pre-build hook for YAML→XML export
- **`wixer_msix_package()`**: Auto-generate AppxManifest from WiX YAML sources

### Quick Build

```bash
cargo build --release              # Build CLI
cargo run -- build wixer.wxs.yaml  # Build MSI from YAML
cargo make msi                     # Build MSI installer (includes VS Code extension)
cargo make msix                    # Build MSIX package
cargo make all                     # Build all packages
```

### VS Code Extension

The `wixer-vscode/` directory contains a VS Code extension providing:
- Syntax highlighting for `.wxs.yaml`, `.wxi.yaml`, `.wxl.yaml` files
- LSP integration via `wixer lsp` command
- Language configuration (comments, brackets, etc.)

```bash
cargo make vscode-package          # Build .vsix package
```

The extension is bundled with the MSI installer as an optional feature that auto-installs when VS Code is detected.

## Key Dependencies

- `clap` – CLI parsing
- `serde_yaml` / `serde_json` – YAML/JSON serialization
- `quick-xml` / `roxmltree` – XML generation/parsing
- `tower-lsp` – Language Server Protocol
- `jsonschema` – YAML validation against JSON Schema
- `directories` – Platform-specific paths
- `tokio` / `reqwest` – Async runtime and HTTP client
- `anyhow` – Error handling
- `tracing` / `tracing-subscriber` – Logging
- `walkdir` – Directory traversal
- `regex` – Pattern matching
- `zip` – Embedded asset extraction (store-only zip)

## Architecture Notes

1. **Build interception**: `wixer build` intercepts YAML files, converts them to `.wixer/intermediates/*.wxs`, then calls real `wix.exe`.
2. **Impersonation mode**: If the binary is renamed to `wix.exe`, it passes through non-wixer commands to the real WiX toolset.
3. **Exit code mapping**: WiX error IDs are mapped to human-readable names via `wix_exit_codes.rs`.
4. **Upgrade pipeline**: The `upgrade/` module uses a transformation pipeline pattern—`PreprocessorPreserver` extracts/restores `<?...?>` directives, and individual `Transformation` trait implementations handle v3→v4 changes.
5. **MSI Simulation**: The `simulation/` module models MSI execution with condition evaluation, action sequencing, and install mode handling (FirstInstall, Upgrade, Uninstall, Repair, Maintenance).
6. **LSP Diagnostics**: The LSP provides real-time diagnostics including orphan component detection, Publish order conflict warnings, and schema validation.
7. **Auto Extension Detection**: `schema_mapping_v3v4.rs` detects required WiX extensions (Util, UI, IIS, NetFx) based on namespace prefixes and element usage.
8. **Embedded Assets**: All schemas and extension metadata are embedded in the binary via `include_bytes!` with a store-only zip for zero-copy access. This enables CI operation without network access.

## Testing

```bash
cargo test --workspace             # Unit tests
cargo nextest run --workspace      # With nextest
cargo clippy --workspace -- -D warnings
```

### Gallery Testing

The `wixer-gallery` binary tests upgrade/conversion against real WiX examples:

```bash
cargo run --bin wixer-gallery -- --help
cargo run --bin wixer-gallery -- build  # Build all gallery examples
```

Examples are configured via `wixer.example.yaml` files in `wixer/examples/configs/`.

## Common Tasks for AI Agents

- **Add a new CLI command**: Create `src/commands/<name>.rs`, add to `mod.rs`, wire up in `app.rs` `Commands` enum.
- **Modify YAML↔XML conversion**: Edit `yaml_to_wix.rs` or `wix_to_yaml.rs`.
- **Update schema handling**: See `xsd_to_json.rs` and `schema_mapping.rs`.
- **LSP features**: Edit `src/commands/lsp.rs`.
- **Add upgrade transformation**: Create a new struct implementing `Transformation` trait in `src/upgrade/transformations.rs`.
- **VS Code extension**: Edit `wixer-vscode/src/extension.ts` for LSP client, `syntaxes/wix-yaml.tmLanguage.json` for syntax highlighting.
- **Add WiX extension detection**: Update `detect_required_extensions()` in `schema_mapping_v3v4.rs`.

# AI Agent Onboarding

## Project Overview

**Wixer** is a Rust CLI tool that enables authoring WiX (Windows Installer XML) projects using YAML instead of XML. It wraps the official `wix.exe` toolset, transparently converting YAML sources to WiX XML at build time.

## Repository Structure

```
wixer/
├── wixer/                  # Main Rust crate (CLI binary)
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── app.rs          # CLI definition (clap) and command dispatch
│   │   ├── commands/       # Subcommand implementations
│   │   ├── yaml_to_wix.rs  # YAML → WiX XML conversion
│   │   ├── wix_to_yaml.rs  # WiX XML → YAML conversion
│   │   ├── xsd_to_json.rs  # XSD → JSON Schema conversion
│   │   ├── config.rs       # Configuration loading (wixer.yaml)
│   │   ├── dirs.rs         # Platform-specific directories
│   │   └── constants.rs    # File extensions, namespaces
│   └── Cargo.toml
├── wixer-vscode/           # VS Code extension (TypeScript)
├── wix/                    # Vendored WiX Toolset source (reference)
├── CMakeLists.txt          # CMake build for MSIX packaging
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
| `wixer describe <term>` | Lookup WiX properties/variables |
| `wixer lsp` | Start Language Server Protocol server |
| `wixer migrate --target msix` | Convert WiX project to MSIX manifest |
| `wixer upgrade <file>` | Upgrade WiX v3 syntax to v4/v6 |

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
- Downloaded WiX XSD schemas
- Generated JSON schemas for validation
- `describe.json` knowledge base

Run `wixer init` to populate the cache.

## Build System

- **Rust**: Edition 2024, workspace with single `wixer` member
- **CI**: GitHub Actions (`.github/workflows/ci.yml`) runs `cargo fmt`, `clippy`, `nextest`, coverage
- **Packaging**: CMake + CPack generates MSIX via `MakeAppX`

The `CMakeLists.txt` and `Makefile.toml` configurations serve as **dogfooding examples**—Wixer builds its own Windows installers (MSI via WiX, MSIX via MakeAppX) using itself.

### Quick Build

```bash
cargo build --release              # Build CLI
cargo run -- build wixer.wxs.yaml  # Build MSI from YAML
cargo make wix-installer           # Full MSI build task
```

## Key Dependencies

- `clap` – CLI parsing
- `serde_yaml` / `serde_json` – YAML/JSON serialization
- `quick-xml` / `roxmltree` – XML generation/parsing
- `tower-lsp` – Language Server Protocol
- `jsonschema` – YAML validation against JSON Schema
- `directories` – Platform-specific paths

## Architecture Notes

1. **Build interception**: `wixer build` intercepts YAML files, converts them to `.wixer/intermediates/*.wxs`, then calls real `wix.exe`.
2. **Impersonation mode**: If the binary is renamed to `wix.exe`, it passes through non-wixer commands to the real WiX toolset.
3. **Exit code mapping**: WiX error IDs are mapped to human-readable names via `wix_exit_codes.rs`.

## Testing

```bash
cargo test --workspace             # Unit tests
cargo nextest run --workspace      # With nextest
cargo clippy --workspace -- -D warnings
```

## Common Tasks for AI Agents

- **Add a new CLI command**: Create `src/commands/<name>.rs`, add to `mod.rs`, wire up in `app.rs` `Commands` enum.
- **Modify YAML↔XML conversion**: Edit `yaml_to_wix.rs` or `wix_to_yaml.rs`.
- **Update schema handling**: See `xsd_to_json.rs` and `schema_mapping.rs`.
- **LSP features**: Edit `src/commands/lsp.rs`.

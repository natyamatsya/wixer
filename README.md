# Wixer

**Author WiX installers using YAML instead of XML.**

Wixer is a Rust CLI tool that wraps the official WiX Toolset, enabling you to write Windows installers in YAML while transparently converting to WiX XML at build time. It also supports migrating WiX projects to MSIX format.

## Features

- **YAML Authoring** — Write `.wxs.yaml` files instead of verbose XML
- **Transparent Build** — Wixer intercepts `wix.exe` calls and handles YAML conversion automatically
- **WiX v4/v6 Support** — Works with modern WiX Toolset versions
- **MSIX Migration** — Generate `AppxManifest.xml` from WiX sources
- **CMake Integration** — Seamless integration with CMake/CPack workflows
- **Language Server** — LSP support for IDE integration
- **VS Code Extension** — Syntax highlighting and language support for WiX YAML files
- **Schema Validation** — Validate YAML against WiX JSON schemas
- **Auto Extension Detection** — Automatically detects required WiX extensions (Util, UI, etc.)

## Quick Start

```bash
# Install (or build from source)
cargo install --path wixer

# Initialize schemas
wixer init

# Build MSI from YAML
wixer build myproject.wxs.yaml -o myproject.msi

# Or convert YAML to XML and use wix.exe directly
wixer export ./src ./build
wix build ./build/myproject.wxs
```

## File Extensions

| YAML Source | WiX Target | Purpose |
|-------------|------------|---------|
| `.wxs.yaml` | `.wxs` | Main installer source |
| `.wxi.yaml` | `.wxi` | Include files |
| `.wxl.yaml` | `.wxl` | Localization |

## Example

**wixer.wxs.yaml:**
```yaml
Wix:
  xmlns: 'http://wixtoolset.org/schemas/v4/wxs'
  Package:
    Name: 'MyApp'
    Version: '1.0.0.0'
    Manufacturer: 'MyCompany'
    UpgradeCode: 'd0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a'
    Scope: 'perMachine'
    Feature:
      Id: 'Main'
      Title: 'MyApp'
      ComponentGroupRef:
        Id: 'ProductComponents'
```

## Commands

| Command | Description |
|---------|-------------|
| `build` | Build MSI from YAML (intercepts wix.exe) |
| `export` | Convert YAML files to WiX XML |
| `import` | Convert WiX XML files to YAML |
| `migrate` | Migrate WiX project to MSIX |
| `validate` | Validate YAML against schemas |
| `describe` | Lookup WiX properties and variables |
| `upgrade` | Upgrade WiX v3 syntax to v4/v6 |
| `lsp` | Start Language Server Protocol server |
| `init` | Initialize schemas (downloads or extracts embedded) |

See [docs/commands.md](docs/commands.md) for detailed command documentation.

## CMake Integration

Wixer provides CMake modules for seamless CPack integration:

```cmake
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")
include(WixerImpersonate)

# Set up wixer to intercept wix.exe calls
wixer_impersonate(
    WIXER_EXE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe"
    VERBOSE
    INSPECT_PDB
)

# Export YAML to XML as a build step
wixer_add_yaml_sources(wixer_yaml_export
    "${CMAKE_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}/wix"
)

# Auto-generate AppxManifest from YAML for MSIX
wixer_msix_package("${CMAKE_SOURCE_DIR}/myproject.wxs.yaml")
```

See [docs/cmake-integration.md](docs/cmake-integration.md) for full CMake documentation.

## Configuration

Wixer reads `wixer.yaml` from:
1. Walking up from the current directory (project-local)
2. Global config directory

**wixer.yaml:**
```yaml
build:
  verbose: false
  inspect_pdb: false
  real_wix_path: "C:/Program Files/WiX Toolset v6.0/bin/wix.exe"

profiles:
  debug:
    verbose: true
    inspect_pdb: true

logging:
  file: "wixer.log"
```

### Config Locations

| OS | Config Directory | Cache Directory |
|----|------------------|-----------------|
| Windows | `%APPDATA%\natyamatsya\wixer\config` | `%LOCALAPPDATA%\natyamatsya\wixer\cache` |
| Linux | `~/.config/wixer` | `~/.cache/wixer` |
| macOS | `~/Library/Application Support/com.natyamatsya.wixer` | `~/Library/Caches/com.natyamatsya.wixer` |

### CI / Offline Usage

Wixer embeds all required schemas and extension metadata at compile time. In CI environments without network access, `wixer init` automatically extracts embedded assets instead of downloading them.

## Building from Source

```bash
# Build CLI
cargo build --release

# Run tests
cargo test --workspace
```

### Building Installers

Wixer can build its own installers using cargo-make or CMake:

**Using cargo-make:**
```bash
# Build MSI installer (includes VS Code extension)
cargo make msi

# Build MSIX package
cargo make msix

# Build all packages (MSI + MSIX)
cargo make all
```

**Using CMake:**
```bash
# Full MSI build workflow
cmake --workflow --preset wixer-rel-pkg

# MSIX with auto-generated AppxManifest
cmake --workflow --preset wixer-auto-msix
```

### VS Code Extension

The VS Code extension provides syntax highlighting and LSP integration for WiX YAML files:

```bash
# Build the extension
cargo make vscode-package

# Install manually
code --install-extension wixer-vscode/wixer-vscode-*.vsix
```

The extension is also bundled with the MSI installer as an optional feature.

## License

MIT License - see [LICENSE](LICENSE) for details.

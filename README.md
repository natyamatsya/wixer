# Wixer

**Author WiX installers in YAML instead of XML.**

Wixer is a Rust CLI that wraps the official WiX Toolset (v4 and later; tested with v7). It converts `.wxs.yaml` sources to WiX XML at build time and adds tooling around them: validation, a language server, installer analysis and simulation, and MSIX migration.

## Important

This repository is an **experimental project** — a personal playground for exploring YAML authoring, language-server tooling, MSI execution simulation, and MSIX migration on top of the WiX Toolset. It is published as-is, purely for anyone curious about that experimentation. There are **no plans for community support**: no issue triage, no roadmap, no release or stability guarantees, and history may be force-rewritten without notice.

If you need a **stable, production-grade installer toolchain**, please use the official [**WiX Toolset**](https://github.com/wixtoolset/wix) directly, which Wixer wraps and builds upon. If you'd like to support that effort, please consider sponsoring its maintainers at <https://github.com/sponsors/wixtoolset>.

## Features

- **YAML authoring** — `.wxs.yaml`, `.wxi.yaml` and `.wxl.yaml` map 1:1 to `.wxs`, `.wxi` and `.wxl`; import/export converts between them
- **Drop-in build** — `wixer build` converts YAML and calls the real `wix.exe`, adding required extensions (Util, UI, …) automatically; renamed to `wix.exe`, Wixer passes all other commands through
- **Validation** — against JSON schemas generated from the WiX XSDs, embedded in the binary (works offline)
- **Language server + VS Code extension** — diagnostics, hover docs and syntax highlighting for WiX YAML
- **Analysis** — pre-build checks, execution-sequence visualization, MSI install/upgrade/uninstall simulation and `.wixpdb` inspection
- **Migration** — upgrade WiX v3 sources to v4+ and generate MSIX `AppxManifest.xml` from WiX projects
- **CMake integration** — use YAML sources transparently with CPack's WIX generator

## Quick Start

```bash
cargo install --path wixer
```

**myapp.wxs.yaml:**

```yaml
Wix:
  xmlns: http://wixtoolset.org/schemas/v4/wxs
  Package:
    Name: MyApp
    Manufacturer: MyCompany
    Version: 1.0.0.0
    UpgradeCode: d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a
    MediaTemplate:
      EmbedCab: yes
    StandardDirectory:
      Id: ProgramFiles64Folder
      Directory:
        Id: INSTALLFOLDER
        Name: MyApp
        Component:
          Id: MainComponent
          File:
            Source: MyApp.exe
    Feature:
      Id: Main
      ComponentRef:
        Id: MainComponent
```

```bash
wixer validate myapp.wxs.yaml
wixer build myapp.wxs.yaml -o myapp.msi   # accepts all `wix build` options

# Or convert to XML and use wix.exe directly
wixer export . ./xml
wix build ./xml/myapp.wxs -o myapp.msi
```

## Commands

| Command | Purpose |
|---------|---------|
| `build` | Convert YAML sources and run `wix build` |
| `export` / `import` | Convert a directory of YAML ↔ WiX XML (`yaml-to-wix` / `wix-to-yaml` for single files) |
| `validate` | Validate YAML against the embedded (or a given) JSON schema |
| `upgrade` | Upgrade WiX v3 sources to v4+ |
| `migrate --target msix` | Generate an MSIX `AppxManifest.xml` from a WiX project |
| `analyze` | Pre-build checks for common authoring issues |
| `sequence` / `simulate` | Visualize the execution sequence / simulate install modes |
| `inspect-pdb` | Inspect a `.wixpdb` file |
| `describe <term>` | Look up MSI properties, WiX elements and custom actions |
| `lsp` | Start the language server |
| `doc` | Generate documentation from the WiX schemas |
| `assets` | Show or extract the embedded assets |

See [docs/commands.md](docs/commands.md) for options and examples.

## Configuration

Wixer reads `wixer.yaml` from the path in `WIXER_CONFIG`, else the first one found walking up from the current directory, else the global config directory (`%APPDATA%\natyamatsya\wixer\config` on Windows, `~/.config/wixer` on Linux, `~/Library/Application Support/com.natyamatsya.wixer` on macOS).

```yaml
build:
  verbose: false
  inspect_pdb: false
  real_wix_path: "C:/Program Files/WiX Toolset v7.0/bin/wix.exe"  # default: wix on PATH
profiles:
  debug:            # select with --profile debug
    verbose: true
    inspect_pdb: true
logging:
  file: wixer.log
```

## CMake Integration

```cmake
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")
include(WixerImpersonate)

wixer_impersonate(WIXER_EXE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe")
wixer_add_yaml_sources(wixer_yaml_export "${CMAKE_SOURCE_DIR}" "${CMAKE_BINARY_DIR}/wix")
wixer_msix_package("${CMAKE_SOURCE_DIR}/myproject.wxs.yaml")
```

See [docs/cmake-integration.md](docs/cmake-integration.md).

## Building from Source

```bash
cargo build --release
cargo test --workspace

cargo make msi             # MSI installer (bundles the VS Code extension)
cargo make msix            # MSIX package
cargo make vscode-package  # VS Code extension (.vsix) only
cargo make gallery-build   # build the example gallery
```

Wixer builds its own installers from [wixer.wxs.yaml](wixer.wxs.yaml); `cmake --workflow --preset wixer-rel-pkg` does the same via CMake/CPack. More in [docs/](docs/README.md): architecture, embedded assets and testing.

## License

MIT — see [LICENSE](LICENSE).

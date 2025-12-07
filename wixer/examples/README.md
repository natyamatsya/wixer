# Wixer Example Gallery

This directory contains the WiX example gallery, demonstrating WiX v3 to v4 upgrades and YAML conversion.

## Directory Structure

```
examples/
├── WiXInstallerExamples/    # Git submodule with original WiX v3 examples (read-only)
├── configs/                  # Build configuration for each example
│   ├── 00_HelloWorldInstaller.yaml
│   ├── 01_HelloWorldInstallerUpgradable.yaml
│   └── ...
├── wixer-yaml/              # ★ CURATED YAML GALLERY (tracked in git)
│   ├── 00_HelloWorldInstaller/
│   ├── ...
│   └── 07_CSharpCustomAction/  # Modernized with .NET 6/8
└── gallery/                  # Generated output (gitignored)
    ├── wix-v4/              # Upgraded WiX v4 XML (regenerated from submodule)
    └── msi/                 # Built MSI packages
```

- **`wixer-yaml/`** is the curated gallery (tracked in git) - edit examples here
- **`gallery/wix-v4/`** is regenerated from submodule (gitignored) - use `--refresh` to update `wixer-yaml/`

## Quick Start

```bash
# Build MSI packages from the curated YAML gallery
cargo make gallery-build

# Just regenerate wix-v4/ from submodule (no build)
cargo make gallery

# Refresh wixer-yaml/ from upgraded v4 files (for maintainers)
cargo make gallery-refresh
```

## Gallery Commands

| Command | Description |
|---------|-------------|
| `cargo make gallery` | Regenerate `wix-v4/` from submodule |
| `cargo make gallery-build` | Build MSI from curated `wixer-yaml/` |
| `cargo make gallery-refresh` | Update `wixer-yaml/` from `wix-v4/` (review changes!) |
| `cargo make gallery-build-example 00` | Build a specific example |
| `cargo make gallery-verbose` | Build with verbose output |

## Example Configuration

Each example has a configuration file in `configs/` that controls how it's built.

### Configuration Schema

```yaml
# Required
name: "00_HelloWorldInstaller"
description: "Minimal WiX installer"

# Build settings (optional)
defines:                          # Preprocessor variables (-d flags)
  Platform: x64
  MyVar: "value"

extensions:                       # WiX extensions (-ext flags)
  - WixToolset.Util.wixext
  - WixToolset.UI.wixext

sources:                          # Source files (auto-detected if omitted)
  - Product.wxs
  - Fragment.wxs

# Build control
buildable: true                   # Set to false to skip building
skip_reason: "Reason for skip"    # Shown when buildable=false

# External dependencies
requires:
  - path: "path/to/file.dll"
    description: "What this file is"
```

### Example Configurations

**Simple example (00_HelloWorldInstaller.yaml):**
```yaml
name: "00_HelloWorldInstaller"
description: "Minimal WiX installer - the simplest possible MSI package"
buildable: true
```

**With preprocessor variables (02_x86_x64_Installer.yaml):**
```yaml
name: "02_x86_x64_Installer"
description: "Platform-dependent installer"
defines:
  Platform: x64
buildable: false
skip_reason: "Requires platform-specific DLL files"
requires:
  - path: "my_x64.dll"
    description: "64-bit DLL for x64 builds"
```

**With extensions (04_PowerShellPreconditionCustomAction.yaml):**
```yaml
name: "04_PowerShellPreconditionCustomAction"
description: "Uses PowerShell via WixQuietExec"
extensions:
  - WixToolset.Util.wixext
buildable: true
```

## Gallery Output

After running `cargo make gallery-build`:

### wix-v4/
Contains WiX v4 XML files upgraded from v3. Key transformations:
- `Product` → `Package` with merged attributes
- `Directory` → `StandardDirectory` for well-known folders
- `Condition` → `Launch` for launch conditions
- Inner text → `Condition` attribute for `Custom`, `SetProperty`, `Publish`
- `BinaryKey` → `BinaryRef`, `FileKey` → `FileRef`
- Namespace updated to `http://wixtoolset.org/schemas/v4/wxs`
- `Win64` attribute removed (platform determined at build time)

### wixer-yaml/
Contains YAML equivalents of the v4 XML files. Files with preprocessor directives (`<?if?>`, `<?include?>`, etc.) are kept as XML since YAML cannot represent them.

### msi/
Contains built MSI packages for examples that completed successfully.

## Build Status

| Example | Status | Notes |
|---------|--------|-------|
| 00_HelloWorldInstaller | ✓ | Builds successfully |
| 01_HelloWorldInstallerUpgradable | ✓ | Builds successfully |
| 02_x86_x64_Installer | ⊘ | Requires DLL files |
| 03_PassingInstallLocationParameterToInstaller | ✓ | Builds successfully |
| 04_PowerShellPreconditionCustomAction | ✓ | Builds successfully |
| 05_RememberPropertyPattern | ✓ | Builds successfully |
| 06_RememberPropertyPatternComplete | ✓ | Builds successfully |
| 07_CSharpCustomAction | ⊘ | Requires C# project build |
| 08_CppCustomAction | ⊘ | Requires C++ project build |
| 09_InstallerWithUserInterface | ⊘ | WixUI v3→v4 migration needed |
| 10_ASP.NET_Core6_WeatherForecastInstaller | ⊘ | Requires ASP.NET app |

**Legend:** ✓ = builds, ⊘ = skipped (external dependency)

## Modernized Examples

Some examples have been modernized from the original submodule:

| Example | Changes |
|---------|---------|
| [07_CSharpCustomAction](wixer-yaml/07_CSharpCustomAction/) | Upgraded to .NET 6/8 (from .NET Framework 4.7.2) |
| [08_CppCustomAction](wixer-yaml/08_CppCustomAction/) | Upgraded to C++23 with CMake (from WiX SDK + vcxproj) |

These include source code (C#/C++ projects) alongside the WiX YAML files.

## Adding New Examples

1. Add the example to `WiXInstallerExamples/` (or use existing)
2. Create a config file in `configs/<example_name>.yaml`
3. Run `cargo make gallery-build` to test

## Programmatic Access

The configuration schema is defined in `wixer/src/example_config.rs` and can be used programmatically:

```rust
use wixer::example_config::ExampleConfig;

let config = ExampleConfig::load(Path::new("configs/00_HelloWorldInstaller.yaml"))?;
println!("Building: {}", config.name);

// Get build arguments
let args = config.build_args();  // Returns ["-d", "Key=Value", "-ext", "..."]

// Check requirements
let missing = config.check_requirements(example_dir);
if !missing.is_empty() {
    println!("Missing: {:?}", missing);
}
```

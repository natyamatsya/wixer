# Example Configuration Files

This directory contains build configuration files for the WiX example gallery.

Each file is named `<example_name>.yaml` and corresponds to an example in `../WiXInstallerExamples/`.

## Schema

See the main [examples README](../README.md) for the full configuration schema.

## Quick Reference

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name (required) |
| `description` | string | What the example demonstrates |
| `defines` | map | Preprocessor variables (`-d` flags) |
| `extensions` | list | WiX extensions (`-ext` flags) |
| `sources` | list | Source files (auto-detected if omitted) |
| `buildable` | bool | Whether to attempt building (default: true) |
| `expect_build_failure` | bool | If true, a failing build is treated as success (for intentionally-invalid examples) |
| `skip_reason` | string | Reason shown when skipped |
| `requires` | list | External dependencies |
| `prebuild` | list | Prebuild commands run when their `produces` outputs are missing |

## Files

| Config | Example | Notes |
|--------|---------|-----------|
| `00_HelloWorldInstaller.yaml` | Minimal installer | ✓ |
| `01_HelloWorldInstallerUpgradable.yaml` | With MajorUpgrade | ✓ |
| `02_x64_arm64_Installer.yaml` | Multi-arch example | ✓ |
| `03_PassingInstallLocationParameterToInstaller.yaml` | Custom install path | ✓ |
| `04_PowerShellPreconditionCustomAction.yaml` | PowerShell precondition | ✓ |
| `05_RememberPropertyPattern.yaml` | Remember Property pattern | ✓ |
| `06_RememberPropertyPatternComplete.yaml` | Complete Remember Property | ✓ |
| `07_CSharpCustomAction.yaml` | C# custom action | ✓ (uses `prebuild`) |
| `08_CppCustomAction.yaml` | C++ custom action | ✓ (uses `prebuild`) |
| `09_InstallerWithUserInterface.yaml` | Custom WixUI dialogs | ✓ |
| `10_Rust_Axum_WeatherForecastInstaller.yaml` | Rust/Axum app installer | ✓ (uses `prebuild`) |
| `known-pitfalls.yaml` | Known pitfalls gallery | ✓ (expected failure) |

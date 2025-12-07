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
| `skip_reason` | string | Reason shown when skipped |
| `requires` | list | External dependencies |

## Files

| Config | Example | Buildable |
|--------|---------|-----------|
| `00_HelloWorldInstaller.yaml` | Minimal installer | ✓ |
| `01_HelloWorldInstallerUpgradable.yaml` | With MajorUpgrade | ✓ |
| `02_x86_x64_Installer.yaml` | Platform-dependent | ⊘ |
| `03_PassingInstallLocationParameterToInstaller.yaml` | Custom install path | ✓ |
| `04_PowerShellPreconditionCustomAction.yaml` | PowerShell precondition | ✓ |
| `05_RememberPropertyPattern.yaml` | Remember Property pattern | ✓ |
| `06_RememberPropertyPatternComplete.yaml` | Complete Remember Property | ✓ |
| `07_CSharpCustomAction.yaml` | C# custom action | ⊘ |
| `08_CppCustomAction.yaml` | C++ custom action | ⊘ |
| `09_InstallerWithUserInterface.yaml` | Custom WixUI dialogs | ⊘ |
| `10_ASP.NET_Core6_WeatherForecastInstaller.yaml` | IIS web app installer | ⊘ |

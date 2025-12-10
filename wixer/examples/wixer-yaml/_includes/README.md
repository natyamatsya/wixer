# Reusable Include Modules

This directory contains reusable YAML fragments that can be included in WiX projects using wixer's `!include` directive.

## Available Modules

| Module | Description |
|--------|-------------|
| `InstallationStages.wxi.yaml` | Defines installation stage properties (FirstInstall, Upgrading, Uninstalling, Maintenance) |
| `PowerShellDetection.wxi.yaml` | Detects PowerShell installation and version |

## Usage

Include a module in your main `.wxs.yaml` file using the `!include` directive:

```yaml
Wix:
  xmlns: http://wixtoolset.org/schemas/v4/wxs
  Package:
    Name: My Installer
    # ... other attributes ...

    # Include reusable modules
    !include ../_includes/InstallationStages.wxi.yaml
    !include ../_includes/PowerShellDetection.wxi.yaml

    # Your installer-specific content
    Launch:
      Condition: Installed OR (POWERSHELLEXE AND POWERSHELLVERSION >= "3.0")
      Message: You must have PowerShell 3.0 or higher.
```

## How It Works

The `!include` directive performs a **deep merge**:

- **Maps**: Included map entries are merged with existing entries
- **Sequences**: Included items are appended to existing sequences
- **Scalars**: Included values override existing values

This allows you to compose complex installers from reusable building blocks while still customizing specific aspects.

## Creating New Modules

When creating a new include module:

1. Use `.wxi.yaml` extension (WiX Include convention)
2. Add a comment header explaining the module's purpose
3. Document any properties or conditions the module provides
4. Keep modules focused on a single concern

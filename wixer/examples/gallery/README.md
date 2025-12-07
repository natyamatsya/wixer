# Wixer Example Gallery

This gallery contains WiX installer examples converted for use with wixer.

## Structure

- **wix-v4/** — WiX v4 XML files (upgraded from v3)
- **wixer-yaml/** — YAML equivalents for use with wixer

## Examples

| Example | Description |
|---------|-------------|
| [00_HelloWorldInstaller](wix-v4/00_HelloWorldInstaller) | To build [minimum installer](Product.wxs) run in PowerShell: ```PowerShell .\Dow |
| [01_HelloWorldInstallerUpgradable](wix-v4/01_HelloWorldInstallerUpgradable) | Relevant code changes: - the Project **Id** attribute is auto-generated - **Upgr |
| [02_x86_x64_Installer](wix-v4/02_x86_x64_Installer) | - x64 platform should be added to the project using Visual Studio's Configuratio |
| [03_PassingInstallLocationParameterToInstaller](wix-v4/03_PassingInstallLocationParameterToInstaller) | WiX defines a [property element](https://wixtoolset.org/documentation/manual/v3/ |
| [04_PowerShellPreconditionCustomAction](wix-v4/04_PowerShellPreconditionCustomAction) | Continuing from the [previous example](../03_PassingInstallLocationParameterToIn |
| [05_RememberPropertyPattern](wix-v4/05_RememberPropertyPattern) | If the user enters values in the install UI or passes them on the command-line,  |
| [06_RememberPropertyPatternComplete](wix-v4/06_RememberPropertyPatternComplete) | To fix the case not covered in the [previous example](../05_RememberPropertyPatt |
| [07_CSharpCustomAction](wix-v4/07_CSharpCustomAction) | Extensions that [integrate WiX Toolset into Visual Studio](https://marketplace.v |
| [08_CppCustomAction](wix-v4/08_CppCustomAction) | Extensions that [integrate WiX Toolset into Visual Studio](https://marketplace.v |
| [09_InstallerWithUserInterface](wix-v4/09_InstallerWithUserInterface) | In this example, we will reuse the [06_RememberPropertyPatternComplete example]( |
| [10_ASP.NET_Core6_WeatherForecastInstaller](wix-v4/10_ASP.NET_Core6_WeatherForecastInstaller) | A prerequisite for building the "10_WeatherForecastInstaller" project is that [d |

## Usage

```bash
# Build an example with wixer
wixer build wixer-yaml/00_HelloWorldInstaller/Product.wxs.yaml

# Or use the v4 XML directly with wix.exe
wix build wix-v4/00_HelloWorldInstaller/Product.wxs
```

## Rebuilding the Gallery

```bash
# From the wixer directory:
cargo run --bin wixer-gallery -- --clean
```

## Source

Examples are derived from [WiXInstallerExamples](https://github.com/kurtanr/WiXInstallerExamples) by kurtanr.

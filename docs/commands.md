# Wixer Commands

## build

Build an MSI installer from YAML sources. This command intercepts the WiX toolset and transparently converts YAML to XML.

```bash
wixer build <file.wxs.yaml> [options]
```

**Options:**

- `-o, --output <path>` — Output MSI file path
- `--profile <name>` — Use a named profile from wixer.yaml
- `--validate` — Run post-build ICE validation on the generated MSI (fails build on ICE errors)

**Example:**

```bash
wixer build myproject.wxs.yaml -o dist/myproject.msi
```

When running as `wix.exe` (impersonation mode), wixer automatically intercepts build commands and converts any `.wxs.yaml` inputs to XML before passing to the real WiX toolset.

---

## export

Recursively convert YAML WiX sources to XML.

```bash
wixer export <input_dir> <output_dir> [options]
```

**Options:**

- `--attribute-strategy <strategy>` — How to handle attributes: `inline` (default), `prefix`, or `separate`

**Example:**

```bash
wixer export ./src ./build/wix
```

Converts all `.wxs.yaml`, `.wxi.yaml`, and `.wxl.yaml` files in the input directory to their XML equivalents.

---

## import

Recursively convert WiX XML files to YAML.

```bash
wixer import <input_dir> <output_dir>
```

**Example:**

```bash
wixer import ./legacy-wix ./src
```

Useful for migrating existing WiX projects to YAML format.

---

## migrate

Migrate a WiX project to another format (currently supports MSIX).

```bash
wixer migrate --target <target> <input> <output_dir>
```

**Targets:**

- `msix` — Generate AppxManifest.xml for MSIX packaging

**Example:**

```bash
wixer migrate --target msix ./myproject.wxs.yaml ./msix
```

The migrate command extracts product information (Name, Version, Manufacturer, Executable) from WiX sources and generates a valid AppxManifest.xml. It supports both `.wxs` XML and `.wxs.yaml` YAML inputs.

---

## validate

Validate YAML files against WiX JSON schemas.

```bash
wixer validate <file.wxs.yaml>
```

**Options:**

- `--schema <path>` — Use an explicit JSON schema file instead of the embedded schema
- `--schema-dir <dir> --yaml-dir <dir>` — Validate a directory (explicit schemas on disk)

**Example:**

```bash
wixer validate myproject.wxs.yaml
```

Uses the embedded JSON schemas by default.

---

## describe

Look up WiX properties, variables, custom actions, and concepts.

```bash
wixer describe <term>
```

**Example:**

```bash
wixer describe INSTALLFOLDER
wixer describe MajorUpgrade
wixer describe WixQuietExec64      # Custom action from Util extension
wixer describe POWERSHELLVERSION   # Property from Util extension
```

Searches the unified knowledge base including:

- Standard MSI properties and actions
- WiX schema elements and attributes
- Extension-provided custom actions, properties, and sequence entries

---

## upgrade

Upgrade WiX v3 files to v4/v6 syntax.

```bash
wixer upgrade <file>
```

**Example:**

```bash
wixer upgrade legacy/Product.wxs
```

Applies transformations to update deprecated elements and attributes to modern WiX syntax.

---

## init

Initialize wixer (no-op; assets are embedded).

```bash
wixer init [options]
```

**Options:**

- `--fresh` — Ignored (kept for backwards compatibility)

**Example:**

```bash
wixer init --fresh
```

Prints information about embedded asset versions and available embedded schemas.

---

## lsp

Start the WiX Language Server Protocol server.

```bash
wixer lsp
```

Provides IDE integration features for `.wxs.yaml` files:

- **Completions**: WiX elements, attributes, custom actions, and properties
- **Hover documentation**: Descriptions from WiX schemas and extension metadata
- **Diagnostics**:
  - Schema validation errors
  - Orphan component warnings (Components/ComponentGroups not referenced by Features)
  - Publish order conflict warnings (collisions with UI extension events)

---

## inspect-pdb

Inspect a WiX PDB file to examine installer contents.

```bash
wixer inspect-pdb <file.wixpdb> [symbol]
```

**Example:**

```bash
wixer inspect-pdb myproject.wixpdb
wixer inspect-pdb myproject.wixpdb INSTALLFOLDER
```

---

## sequence

Visualize the execution sequence of installer actions.

```bash
wixer sequence <file>
```

Generates a visualization of the InstallExecuteSequence and InstallUISequence.

---

## yaml-to-wix

Convert a single YAML file to WiX XML.

```bash
wixer yaml-to-wix <input.wxs.yaml> <output.wxs>
```

Lower-level command for single-file conversion. Prefer `export` for batch operations.

---

## wix-to-yaml

Convert a single WiX XML file to YAML.

```bash
wixer wix-to-yaml <input.wxs> <output.wxs.yaml>
```

Lower-level command for single-file conversion. Prefer `import` for batch operations.

---

## xsd-to-json

Convert WiX XSD schemas to JSON Schema format.

```bash
wixer xsd-to-json <input.xsd> <output.json>
```

This is primarily a development tool used to (re)generate schemas for embedding.

---

## doc

Generate documentation from WiX XSD schemas.

```bash
wixer doc [--output-dir <dir>] [--format <format>]
```

Generates documentation from the embedded WiX XSD schemas.

---

## simulate

Simulate MSI installer execution to preview what actions would run in different installation scenarios.

```bash
wixer simulate <file> [options]
```

**Options:**

- `--mode <mode>` — Installation mode: `first-install`, `upgrade`, `uninstall`, `repair`, `maintenance`
- `--verbose` — Show detailed action information

**Example:**

```bash
wixer simulate myproject.wxs.yaml --mode upgrade
wixer simulate myproject.wxs.yaml --mode uninstall --verbose
```

Parses the WiX project and simulates execution flow, showing which custom actions and standard actions would run based on conditions and install mode properties.

---

## map-schemas

Map WiX schemas to MSIX schemas for migration analysis.

```bash
wixer map-schemas
```

Analyzes schema compatibility between WiX and MSIX formats.

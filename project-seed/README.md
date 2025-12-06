# XSD to JSON Schema Converter

Automated tool to convert XML Schema Definition (XSD) files to JSON Schema format for YAML validation. Designed specifically for WiX v4 schemas but works with any XSD files.

## 🎯 Purpose

This tool enables you to:
1. **Convert XSD schemas to JSON Schema** - Automated, robust conversion
2. **Write installers in YAML** - Clean, readable, comment-friendly format
3. **Validate YAML before conversion** - Catch errors early
4. **Convert YAML to XML** - Generate WiX XML files on the fly

## 🚀 Quick Start

### Installation

```bash
# Install dependencies
pip install lxml jsonschema pyyaml

# Or use requirements.txt
pip install -r requirements.txt
```

### Basic Usage

```bash
# Download and convert all WiX schemas
python xsd_to_jsonschema.py --download-wix

# This creates:
# - schemas/          (Downloaded XSD files)
# - json-schemas/     (Generated JSON schemas)
```

## 📖 Detailed Usage

### Converting XSD to JSON Schema

**Download and convert WiX schemas:**
```bash
python xsd_to_jsonschema.py --download-wix --output-dir wix-json-schemas
```

**Convert a single XSD file:**
```bash
python xsd_to_jsonschema.py -i myschema.xsd -o myschema.schema.json
```

**Convert with different attribute handling:**
```bash
# Inline (default): attributes become regular properties
python xsd_to_jsonschema.py -i schema.xsd --attribute-strategy inline

# Prefix: attributes get @ prefix (@Id, @Name)
python xsd_to_jsonschema.py -i schema.xsd --attribute-strategy prefix

# Separate: attributes in separate object
python xsd_to_jsonschema.py -i schema.xsd --attribute-strategy separate
```

**Strict mode (enforce required fields):**
```bash
python xsd_to_jsonschema.py -i schema.xsd --strict
```

### Validating YAML Files

**Validate single file:**
```bash
python validate_yaml.py wix.schema.json myinstaller.yaml
```

**Validate entire directory:**
```bash
python validate_yaml.py --schema-dir json-schemas --yaml-dir installers
```

**Using check-jsonschema (alternative):**
```bash
# Install check-jsonschema
pip install check-jsonschema

# Validate
check-jsonschema --schemafile wix.schema.json installer.yaml
```

## 🔧 Attribute Handling Strategies

The converter offers three strategies for handling XML attributes:

### 1. Inline (Default) ✅ Recommended

XML attributes become regular YAML properties:

**XML:**
```xml
<Product Id="*" Name="MyApp" Version="1.0.0">
```

**YAML:**
```yaml
Product:
  Id: "*"
  Name: "MyApp"
  Version: "1.0.0"
```

**Pros:** Clean, readable, most intuitive
**Cons:** No distinction between attributes and elements

### 2. Prefix

Attributes get `@` prefix:

**YAML:**
```yaml
Product:
  '@Id': "*"
  '@Name': "MyApp"
  '@Version': "1.0.0"
```

**Pros:** Clear distinction, familiar from XML
**Cons:** Less readable, requires quotes

### 3. Separate

Attributes in separate object:

**YAML:**
```yaml
Product:
  attributes:
    Id: "*"
    Name: "MyApp"
    Version: "1.0.0"
  elements:
    # child elements here
```

**Pros:** Maximum clarity
**Cons:** Most verbose

## 📋 Example: WiX Installer in YAML

Here's what a WiX installer looks like in YAML (using inline strategy):

```yaml
# Simple WiX Installer
# Comments are supported in YAML!

Wix:
  xmlns: "http://wixtoolset.org/schemas/v4/wxs"
  
  Product:
    Id: "*"
    Name: "My Application"
    Language: "1033"
    Version: "1.0.0.0"
    Manufacturer: "ACME Corporation"
    UpgradeCode: "12345678-1234-1234-1234-123456789012"
    
    Package:
      InstallerVersion: "500"
      Compressed: "yes"
      InstallScope: "perMachine"
      Description: "Installs My Application"
      Comments: "My Application Installer"
    
    MajorUpgrade:
      DowngradeErrorMessage: "A newer version is already installed."
    
    MediaTemplate:
      EmbedCab: "yes"
    
    # Define the directory structure
    Directory:
      Id: "TARGETDIR"
      Name: "SourceDir"
      
      Directory:
        Id: "ProgramFiles64Folder"
        
        Directory:
          Id: "INSTALLFOLDER"
          Name: "MyApp"
          
          Component:
            Id: "MainExecutable"
            Guid: "*"
            
            File:
              - Id: "MainExe"
                Source: "bin\\MyApp.exe"
                KeyPath: "yes"
              
              - Id: "ConfigFile"
                Source: "config\\app.config"
    
    # Install the component
    Feature:
      Id: "ProductFeature"
      Title: "Main Application"
      Level: "1"
      
      ComponentRef:
        Id: "MainExecutable"
```

## 🔄 Complete Workflow

### 1. Convert XSD to JSON Schema

```bash
python xsd_to_jsonschema.py --download-wix
```

Creates:
- `json-schemas/wix.schema.json`
- `json-schemas/bal_bal.schema.json`
- `json-schemas/iis_iis.schema.json`
- etc.

### 2. Write Your Installer in YAML

Create `myapp.yaml`:

```yaml
Product:
  Id: "*"
  Name: "MyApp"
  Version: "1.0.0"
  # ... rest of installer definition
```

### 3. Validate YAML

```bash
python validate_yaml.py json-schemas/wix.schema.json myapp.yaml
```

Output:
```
✓ YAML is valid!
```

### 4. Convert YAML to WiX XML

```bash
python yaml_to_wix.py myapp.yaml myapp.wxs
```

Creates valid WiX XML file.

### 5. Build with WiX

```bash
wix build myapp.wxs -out myapp.msi
```

## 📁 Generated Files

After running the converter, you'll have:

```
project/
├── schemas/                    # Downloaded XSD files
│   ├── wix.xsd
│   ├── bal_bal.xsd
│   └── ...
├── json-schemas/               # Generated JSON schemas
│   ├── wix.schema.json
│   ├── bal_bal.schema.json
│   └── ...
├── installers/                 # Your YAML installers
│   ├── myapp.yaml
│   └── another.yaml
└── output/                     # Generated WiX XML
    ├── myapp.wxs
    └── another.wxs
```

## 🎨 JSON Schema Features

The generated JSON schemas include:

### Type Definitions
- All XSD types converted to JSON Schema types
- Complex types → objects with properties
- Simple types → primitives with constraints

### Validation Rules
- Required vs optional fields
- Enumerations
- Pattern matching (regex)
- Length constraints
- Numeric ranges
- Array cardinality (min/max items)

### Documentation
- XSD annotations preserved as descriptions
- Helps IDE autocomplete and validation

### Example Schema Fragment

```json
{
  "$schema": "https://json-schema.org/draft-07/schema#",
  "type": "object",
  "definitions": {
    "Product": {
      "type": "object",
      "properties": {
        "Id": {
          "type": "string",
          "description": "Unique identifier for the product"
        },
        "Name": {
          "type": "string",
          "description": "The descriptive name of the product"
        },
        "Version": {
          "type": "string",
          "pattern": "^\\d+\\.\\d+\\.\\d+(\\.\\d+)?$",
          "description": "Product version in x.x.x.x format"
        }
      },
      "required": ["Id", "Name", "Version"]
    }
  }
}
```

## ⚙️ Advanced Options

### Custom Root Element

Specify a root element to focus the schema:

```bash
python xsd_to_jsonschema.py -i wix.xsd --root-element Product
```

### Exclude Descriptions

Generate smaller schemas without documentation:

```bash
python xsd_to_jsonschema.py -i schema.xsd --no-descriptions
```

### Process Multiple Files

```bash
for file in schemas/*.xsd; do
  python xsd_to_jsonschema.py -i "$file" --output-dir json-schemas
done
```

## 🐛 Troubleshooting

### "No matching schema found"

The validator looks for schemas with matching names. Ensure:
- Schema file exists: `wix.schema.json`
- YAML file exists: `myapp.yaml`
- Or explicitly specify: `python validate_yaml.py schema.json file.yaml`

### "Validation failed: ... is not valid"

Check the error message for the specific property that failed. Common issues:
- Missing required fields
- Wrong data type (string vs integer)
- Invalid enum value
- Pattern mismatch

### "Cannot find type definition"

The XSD may reference types from imported schemas. Ensure:
- All referenced XSD files are downloaded
- The converter processes includes/imports correctly

## 🔮 Future Enhancements

Potential improvements:

- [ ] GUI for schema conversion
- [ ] Interactive YAML editor with validation
- [ ] Visual schema explorer
- [ ] YAML↔XML round-trip testing
- [ ] Template library for common patterns
- [ ] Integration with WiX build tools

## 📚 Resources

### WiX Toolset
- [WiX v4 Documentation](https://wixtoolset.org/docs/intro/)
- [WiX GitHub Repository](https://github.com/wixtoolset/wix)
- [WiX Schema Files](https://github.com/wixtoolset/wix/tree/develop/src/api/wix)

### JSON Schema
- [JSON Schema Specification](https://json-schema.org/)
- [Understanding JSON Schema](https://json-schema.org/understanding-json-schema/)
- [JSON Schema Validator](https://www.jsonschemavalidator.net/)

### YAML
- [YAML Specification](https://yaml.org/spec/)
- [YAML Lint](http://www.yamllint.com/)

## 🤝 Contributing

Suggestions and improvements welcome! Areas of interest:
- Better handling of XML namespaces
- More sophisticated xs:choice conversion
- Improved documentation generation
- Additional validation rules

## 📄 License

MIT License - Feel free to use and modify as needed.

## 🙏 Acknowledgments

- WiX Toolset team for excellent schemas
- lxml for XSD parsing
- jsonschema for validation

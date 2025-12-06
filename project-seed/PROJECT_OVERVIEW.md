# XSD to YAML Schema Converter - Project Files

## 📦 Complete Toolkit

This package contains everything needed to convert XSD schemas to JSON Schema and work with YAML-based installers.

## 🗂️ Files Included

### Core Conversion Tools

1. **xsd_to_jsonschema.py** (400+ lines)
   - Main conversion engine
   - Parses XSD files using lxml
   - Generates JSON Schema (Draft 7)
   - Handles complex types, simple types, sequences, choices
   - Supports multiple attribute strategies
   - Downloads WiX schemas automatically
   
2. **yaml_to_wix.py** (300+ lines)
   - Converts YAML to WiX XML format
   - Handles inline, prefix, and separate attribute strategies
   - Pretty-prints XML output
   - Batch conversion support

3. **validate_yaml.py** (200+ lines)
   - Validates YAML against JSON Schema
   - Single file or directory validation
   - Detailed error reporting
   - Uses jsonschema library

### Documentation

4. **README.md**
   - Comprehensive documentation
   - Usage examples
   - Attribute strategies explained
   - Complete workflow guide
   - Troubleshooting section

5. **QUICKSTART.md**
   - 5-minute getting started guide
   - Step-by-step instructions
   - Tips and tricks
   - CI/CD integration examples

### Examples & Demo

6. **example_installer.yaml**
   - Complete WiX installer example in YAML
   - Demonstrates all major features:
     - Product definition
     - Directory structure
     - Components and files
     - Shortcuts (desktop, start menu)
     - Features
     - Icons and properties
   - Fully commented

7. **demo.py**
   - Interactive demonstration script
   - Shows complete workflow:
     1. Download WiX schema
     2. Convert to JSON Schema
     3. Create YAML installer
     4. Validate YAML
     5. Convert to XML
   - Can be run standalone for testing

### Configuration

8. **requirements.txt**
   - Python dependencies:
     - lxml >= 4.9.0 (XSD parsing)
     - jsonschema >= 4.17.0 (validation)
     - pyyaml >= 6.0 (YAML processing)

## 🎯 Quick Command Reference

```bash
# Setup
pip install -r requirements.txt

# Download & convert WiX schemas
python xsd_to_jsonschema.py --download-wix

# Convert single XSD
python xsd_to_jsonschema.py -i schema.xsd -o output.json

# Validate YAML
python validate_yaml.py schema.json installer.yaml

# Convert YAML to XML
python yaml_to_wix.py installer.yaml output.wxs

# Run demo
python demo.py
```

## 🔧 Key Features

### XSD Converter
- ✅ Handles complexType and simpleType
- ✅ Processes sequences, choices, and all groups
- ✅ Converts restrictions and enumerations
- ✅ Preserves XSD annotations as descriptions
- ✅ Supports xs:include and xs:import
- ✅ Configurable attribute handling
- ✅ Strict mode for required fields

### YAML Validator
- ✅ JSON Schema Draft 7 validation
- ✅ Detailed error messages with paths
- ✅ Batch validation of directories
- ✅ Summary reports

### XML Converter
- ✅ Clean XML generation
- ✅ Pretty-printed output
- ✅ Multiple attribute strategies
- ✅ Handles arrays and nested structures

## 📊 File Statistics

```
xsd_to_jsonschema.py    ~450 lines    Conversion engine
yaml_to_wix.py          ~320 lines    XML generator
validate_yaml.py        ~220 lines    Validation tool
demo.py                 ~280 lines    Demo script
README.md               ~350 lines    Full documentation
QUICKSTART.md           ~250 lines    Quick guide
example_installer.yaml  ~130 lines    Full example
requirements.txt          3 lines     Dependencies
--------------------------------
Total                  ~2000+ lines   Complete toolkit
```

## 🚀 Usage Patterns

### Pattern 1: Convert Existing XSD Schemas

```bash
# Convert your custom schemas
python xsd_to_jsonschema.py -i myschema.xsd -o myschema.json
python xsd_to_jsonschema.py -i yourschema.xsd -o yourschema.json
```

### Pattern 2: WiX Development Workflow

```bash
# One-time setup
python xsd_to_jsonschema.py --download-wix

# For each installer
vi installer.yaml
python validate_yaml.py json-schemas/wix.schema.json installer.yaml
python yaml_to_wix.py installer.yaml installer.wxs
wix build installer.wxs -out installer.msi
```

### Pattern 3: Batch Processing

```bash
# Validate all YAML files
python validate_yaml.py --schema-dir json-schemas --yaml-dir installers

# Convert all to XML
python yaml_to_wix.py --input-dir installers --output-dir wxs-files

# Build all installers
for file in wxs-files/*.wxs; do
  wix build "$file" -out "msi/$(basename "$file" .wxs).msi"
done
```

### Pattern 4: CI/CD Integration

```yaml
# .github/workflows/build.yml
- name: Setup
  run: pip install -r requirements.txt

- name: Convert schemas (cached)
  run: python xsd_to_jsonschema.py --download-wix
  if: steps.cache.outputs.cache-hit != 'true'

- name: Validate installer
  run: python validate_yaml.py json-schemas/wix.schema.json installer.yaml

- name: Generate WiX XML
  run: python yaml_to_wix.py installer.yaml installer.wxs

- name: Build installer
  run: wix build installer.wxs -out installer.msi
```

## 🎓 Learning Path

1. **Start with demo.py** - See the workflow in action
2. **Read QUICKSTART.md** - Get hands-on quickly
3. **Study example_installer.yaml** - Learn by example
4. **Refer to README.md** - Deep dive into features
5. **Customize for your needs** - Adapt the tools

## 🔍 Technical Details

### XSD to JSON Schema Mappings

**Types:**
- xs:string → string
- xs:integer → integer
- xs:boolean → boolean
- xs:decimal → number

**Structures:**
- xs:complexType → object with properties
- xs:sequence → ordered properties
- xs:choice → oneOf
- xs:all → unordered properties

**Constraints:**
- xs:enumeration → enum
- xs:pattern → pattern (regex)
- xs:minLength/maxLength → minLength/maxLength
- minOccurs/maxOccurs → minItems/maxItems

**Attributes:**
Three strategies:
1. Inline: Regular properties
2. Prefix: @attribute
3. Separate: attributes object

## 📝 Customization

### Modify Attribute Handling

Edit `ATTRIBUTE_PROPERTIES` in `yaml_to_wix.py`:
```python
ATTRIBUTE_PROPERTIES = {
    'Id', 'Name', 'Version',
    'YourCustomAttribute',  # Add your attributes
    # ...
}
```

### Adjust Type Mappings

Edit `TYPE_MAPPING` in `xsd_to_jsonschema.py`:
```python
TYPE_MAPPING = {
    'string': 'string',
    'customType': 'string',  # Add your types
    # ...
}
```

### Extend Validation

Add custom validators in `validate_yaml.py`:
```python
# Add custom format checkers
# Add custom validators
# Extend error reporting
```

## 🤝 Integration with Other Tools

### VS Code
- Install YAML extension
- Add schema comment to YAML files
- Get autocomplete and validation

### PyCharm/IntelliJ
- Configure JSON Schema mappings
- Enable YAML support
- Get inline validation

### Pre-commit Hooks
```yaml
# .pre-commit-config.yaml
- repo: local
  hooks:
    - id: validate-yaml
      name: Validate YAML installers
      entry: python validate_yaml.py
      language: system
      files: \.yaml$
```

## 💡 Best Practices

1. **Version control schemas** - Commit JSON schemas to repo
2. **Validate in CI** - Catch errors early
3. **Use templates** - Create reusable base installers
4. **Document with comments** - YAML supports comments!
5. **Test incrementally** - Validate after each change
6. **Keep it organized** - Separate schemas, YAML, and XML

## 🎉 What Makes This Special

✅ **Automated** - No manual schema conversion
✅ **Robust** - Handles complex XSD features
✅ **Complete** - Full workflow from XSD to MSI
✅ **Well-documented** - Extensive guides and examples
✅ **Tested** - Includes demo and validation
✅ **Flexible** - Multiple configuration options
✅ **Production-ready** - Can be used immediately

## 📞 Support

For issues or questions:
1. Check QUICKSTART.md for common issues
2. Review README.md troubleshooting section
3. Run demo.py to verify setup
4. Check WiX documentation for WiX-specific questions

---

**Created:** December 2025
**Status:** Production ready
**License:** Free to use and modify

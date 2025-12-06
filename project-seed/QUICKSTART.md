# Quick Start Guide

Get up and running with YAML-based WiX installers in 5 minutes!

## 📦 Step 1: Install Dependencies

```bash
pip install -r requirements.txt
```

This installs:
- `lxml` - For parsing XSD files
- `jsonschema` - For validating YAML
- `pyyaml` - For reading/writing YAML

## 🔄 Step 2: Convert XSD to JSON Schema

Download and convert all WiX v4 schemas:

```bash
python xsd_to_jsonschema.py --download-wix
```

**What this does:**
- Downloads WiX XSD files to `schemas/` directory
- Converts them to JSON Schema in `json-schemas/` directory
- Takes ~30 seconds

**Output:**
```
Downloading WiX v4 schemas...
Downloading wix.xsd...
Downloading wixloc.xsd...
...
Downloaded 14 schema files

Converting schemas/wix.xsd...
✓ Generated: json-schemas/wix.schema.json
  - 147 type definitions
  - 5 root properties
...
Conversion complete!
```

## ✍️ Step 3: Write Your Installer in YAML

Create `myapp.yaml`:

```yaml
Wix:
  xmlns: "http://wixtoolset.org/schemas/v4/wxs"
  
  Product:
    Id: "*"
    Name: "My Application"
    Version: "1.0.0"
    Manufacturer: "Your Company"
    UpgradeCode: "YOUR-GUID-HERE"
    
    Package:
      InstallerVersion: "500"
      Compressed: "yes"
      
    MediaTemplate:
      EmbedCab: "yes"
    
    Directory:
      Id: "TARGETDIR"
      Name: "SourceDir"
      
      Directory:
        Id: "ProgramFiles64Folder"
        Directory:
          Id: "INSTALLFOLDER"
          Name: "MyApp"
          
          Component:
            Id: "MainComponent"
            Guid: "*"
            File:
              Id: "MainExe"
              Source: "MyApp.exe"
              KeyPath: "yes"
    
    Feature:
      Id: "Complete"
      Title: "My Application"
      Level: "1"
      ComponentRef:
        Id: "MainComponent"
```

**Or use the example:**
```bash
# An example is already provided
cat example_installer.yaml
```

## ✅ Step 4: Validate Your YAML

```bash
python validate_yaml.py json-schemas/wix.schema.json myapp.yaml
```

**Success:**
```
Validating myapp.yaml against wix.schema.json...

✓ YAML is valid!
```

**If there are errors:**
```
✗ YAML is invalid:

  • Product.Version: '1.0' does not match pattern '^\d+\.\d+\.\d+(\.\d+)?$'
  • Package: 'InstallerVersion' is a required property
```

Fix the issues and validate again!

## 🔨 Step 5: Convert YAML to WiX XML

```bash
python yaml_to_wix.py myapp.yaml myapp.wxs
```

**Output:**
```
Converting myapp.yaml -> myapp.wxs
✓ Successfully converted to myapp.wxs
```

**View the generated XML:**
```bash
cat myapp.wxs
```

```xml
<?xml version="1.0" ?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Product Id="*" Name="My Application" Version="1.0.0" ...>
    <Package InstallerVersion="500" Compressed="yes"/>
    <MediaTemplate EmbedCab="yes"/>
    <Directory Id="TARGETDIR" Name="SourceDir">
      ...
    </Directory>
    <Feature Id="Complete" Title="My Application" Level="1">
      <ComponentRef Id="MainComponent"/>
    </Feature>
  </Product>
</Wix>
```

## 🚀 Step 6: Build Your Installer

Use WiX to compile the XML:

```bash
wix build myapp.wxs -out myapp.msi
```

That's it! You now have `myapp.msi` ready to distribute.

## 🎯 Complete Workflow

```bash
# 1. Set up (one time)
pip install -r requirements.txt
python xsd_to_jsonschema.py --download-wix

# 2. For each installer
vi myapp.yaml                                              # Write installer
python validate_yaml.py json-schemas/wix.schema.json myapp.yaml  # Validate
python yaml_to_wix.py myapp.yaml myapp.wxs                # Convert
wix build myapp.wxs -out myapp.msi                        # Build
```

## 💡 Tips

### Use an IDE with JSON Schema Support

**VS Code:**
1. Install "YAML" extension
2. Add to `myapp.yaml`:
   ```yaml
   # yaml-language-server: $schema=json-schemas/wix.schema.json
   ```
3. Get autocomplete and validation as you type!

**IntelliJ/PyCharm:**
1. Settings → Languages & Frameworks → Schemas and DTDs → JSON Schema Mappings
2. Add schema `json-schemas/wix.schema.json` for `*.yaml` files

### Write Reusable Templates

Create `templates/base-installer.yaml`:
```yaml
Wix:
  xmlns: "http://wixtoolset.org/schemas/v4/wxs"
  Product:
    Language: "1033"
    Package:
      InstallerVersion: "500"
      Compressed: "yes"
      InstallScope: "perMachine"
    MediaTemplate:
      EmbedCab: "yes"
```

Then extend it in your installers.

### Validate in CI/CD

Add to your build pipeline:

```yaml
# .github/workflows/build.yml
- name: Validate installer
  run: python validate_yaml.py json-schemas/wix.schema.json installer.yaml

- name: Build installer
  run: |
    python yaml_to_wix.py installer.yaml installer.wxs
    wix build installer.wxs -out installer.msi
```

### Handle Multiple Installers

```bash
# Validate all installers
python validate_yaml.py --schema-dir json-schemas --yaml-dir installers

# Convert all installers
python yaml_to_wix.py --input-dir installers --output-dir wxs-files

# Build all installers
for file in wxs-files/*.wxs; do
  wix build "$file" -out "output/$(basename "$file" .wxs).msi"
done
```

## 🆘 Troubleshooting

### "ModuleNotFoundError: No module named 'lxml'"
```bash
pip install -r requirements.txt
```

### "No such file or directory: 'schemas/wix.xsd'"
```bash
python xsd_to_jsonschema.py --download-wix
```

### "YAML is invalid: ... is a required property"
Check the schema requirements. Some fields are mandatory in WiX.

### "Cannot find 'wix' command"
Install WiX toolset from https://wixtoolset.org/

## 📚 Next Steps

- Read the [full README](README.md) for advanced options
- Check [example_installer.yaml](example_installer.yaml) for a complete example
- Explore [WiX documentation](https://wixtoolset.org/docs/) for features
- Join the WiX community for help

## 🎉 Benefits

**Why YAML instead of XML?**

✅ **More readable** - Less verbose, cleaner structure
✅ **Comments** - Document your installer inline
✅ **Validation** - Catch errors before building
✅ **Version control** - Easier to review diffs
✅ **IDE support** - Autocomplete and inline docs
✅ **Reusable** - Template and compose installers

Happy installer building! 🚀

#!/usr/bin/env python3
"""
Demo Script - Complete XSD to YAML Workflow
Demonstrates the full conversion and validation pipeline.

This script:
1. Downloads a sample WiX XSD file
2. Converts it to JSON Schema
3. Creates a sample YAML installer
4. Validates the YAML against the schema
5. Converts YAML back to WiX XML
"""

import os
import sys
import tempfile
import urllib.request
from pathlib import Path

# Add current directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from xsd_to_jsonschema import XSDToJSONSchemaConverter
from validate_yaml import validate_yaml_file
from yaml_to_wix import YAMLToWiXConverter, load_yaml


def print_header(title):
    """Print section header."""
    print("\n" + "="*70)
    print(f"  {title}")
    print("="*70 + "\n")


def demo_conversion():
    """Run the complete demo."""
    
    print_header("XSD to YAML Installer Workflow Demo")
    
    # Create temporary directories
    with tempfile.TemporaryDirectory() as tmpdir:
        tmppath = Path(tmpdir)
        
        # Directory structure
        schemas_dir = tmppath / "schemas"
        json_schemas_dir = tmppath / "json-schemas"
        installers_dir = tmppath / "installers"
        output_dir = tmppath / "output"
        
        for d in [schemas_dir, json_schemas_dir, installers_dir, output_dir]:
            d.mkdir(exist_ok=True)
        
        # Step 1: Download sample XSD
        print_header("Step 1: Download Sample WiX Schema")
        
        wix_xsd_url = "https://raw.githubusercontent.com/wixtoolset/wix/develop/src/api/wix/wix.xsd"
        wix_xsd_path = schemas_dir / "wix.xsd"
        
        print(f"Downloading: {wix_xsd_url}")
        print(f"Saving to: {wix_xsd_path}")
        
        try:
            urllib.request.urlretrieve(wix_xsd_url, wix_xsd_path)
            print("✓ Download successful!")
            print(f"  File size: {wix_xsd_path.stat().st_size:,} bytes")
        except Exception as e:
            print(f"✗ Download failed: {e}")
            print("  Continuing with demo using mock data...")
            return False
        
        # Step 2: Convert XSD to JSON Schema
        print_header("Step 2: Convert XSD to JSON Schema")
        
        json_schema_path = json_schemas_dir / "wix.schema.json"
        
        print(f"Converting: {wix_xsd_path}")
        print(f"Output: {json_schema_path}")
        
        try:
            converter = XSDToJSONSchemaConverter(
                attribute_strategy='inline',
                include_descriptions=True,
                strict_mode=False
            )
            
            schema_tree = converter.load_schema(str(wix_xsd_path))
            converter.parse_schema(schema_tree, str(wix_xsd_path))
            json_schema = converter.generate_schema()
            
            import json
            with open(json_schema_path, 'w', encoding='utf-8') as f:
                json.dump(json_schema, f, indent=2)
            
            num_defs = len(json_schema.get('definitions', {}))
            num_props = len(json_schema.get('properties', {}))
            
            print("✓ Conversion successful!")
            print(f"  Type definitions: {num_defs}")
            print(f"  Root properties: {num_props}")
            print(f"  Schema size: {json_schema_path.stat().st_size:,} bytes")
            
        except Exception as e:
            print(f"✗ Conversion failed: {e}")
            import traceback
            traceback.print_exc()
            return False
        
        # Step 3: Create sample YAML installer
        print_header("Step 3: Create Sample YAML Installer")
        
        yaml_content = """# Simple WiX Installer in YAML
Wix:
  xmlns: "http://wixtoolset.org/schemas/v4/wxs"
  
  Product:
    Id: "*"
    Name: "Demo Application"
    Version: "1.0.0"
    Language: "1033"
    Manufacturer: "Demo Corp"
    UpgradeCode: "12345678-1234-1234-1234-123456789012"
    
    Package:
      InstallerVersion: "500"
      Compressed: "yes"
      Description: "Demo Application Installer"
    
    MediaTemplate:
      EmbedCab: "yes"
    
    Directory:
      Id: "TARGETDIR"
      Name: "SourceDir"
      
      Directory:
        Id: "ProgramFiles64Folder"
        
        Directory:
          Id: "INSTALLFOLDER"
          Name: "DemoApp"
          
          Component:
            Id: "MainComponent"
            Guid: "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE"
            
            File:
              Id: "MainExe"
              Source: "DemoApp.exe"
              KeyPath: "yes"
    
    Feature:
      Id: "Complete"
      Title: "Demo Application"
      Level: "1"
      
      ComponentRef:
        Id: "MainComponent"
"""
        
        yaml_path = installers_dir / "demo.yaml"
        
        with open(yaml_path, 'w', encoding='utf-8') as f:
            f.write(yaml_content)
        
        print(f"Created: {yaml_path}")
        print(f"  Lines: {len(yaml_content.splitlines())}")
        print(f"  Size: {yaml_path.stat().st_size} bytes")
        print("\nYAML Content:")
        print("-" * 70)
        print(yaml_content[:500] + "..." if len(yaml_content) > 500 else yaml_content)
        print("-" * 70)
        
        # Step 4: Validate YAML against schema
        print_header("Step 4: Validate YAML Against Schema")
        
        print(f"Validating: {yaml_path}")
        print(f"Schema: {json_schema_path}")
        
        is_valid, errors = validate_yaml_file(str(yaml_path), str(json_schema_path))
        
        if is_valid:
            print("✓ YAML is valid!")
            print("  No schema violations found")
        else:
            print("✗ YAML validation failed:")
            for error in errors[:5]:  # Show first 5 errors
                print(error)
            if len(errors) > 5:
                print(f"  ... and {len(errors) - 5} more errors")
            return False
        
        # Step 5: Convert YAML to WiX XML
        print_header("Step 5: Convert YAML to WiX XML")
        
        xml_path = output_dir / "demo.wxs"
        
        print(f"Converting: {yaml_path}")
        print(f"Output: {xml_path}")
        
        try:
            yaml_data = load_yaml(str(yaml_path))
            converter = YAMLToWiXConverter(attribute_strategy='inline')
            xml_root = converter.convert(yaml_data)
            converter.save_xml(xml_root, str(xml_path))
            
            print("✓ Conversion successful!")
            print(f"  Size: {xml_path.stat().st_size:,} bytes")
            
            # Show first few lines
            with open(xml_path, 'r', encoding='utf-8') as f:
                lines = f.readlines()[:15]
            
            print("\nGenerated XML (first 15 lines):")
            print("-" * 70)
            print(''.join(lines))
            print("...")
            print("-" * 70)
            
        except Exception as e:
            print(f"✗ Conversion failed: {e}")
            import traceback
            traceback.print_exc()
            return False
        
        # Summary
        print_header("Demo Complete!")
        
        print("Summary of generated files:")
        print(f"  ✓ XSD Schema: {wix_xsd_path}")
        print(f"  ✓ JSON Schema: {json_schema_path}")
        print(f"  ✓ YAML Installer: {yaml_path}")
        print(f"  ✓ WiX XML: {xml_path}")
        
        print("\nWorkflow verified:")
        print("  1. XSD → JSON Schema ✓")
        print("  2. YAML authored ✓")
        print("  3. YAML validated ✓")
        print("  4. YAML → WiX XML ✓")
        
        print("\nNext steps:")
        print("  - Review the generated files")
        print("  - Compile the XML with: wix build demo.wxs -out demo.msi")
        print("  - Create your own YAML installers!")
        
        print("\nFiles are in temporary directory (will be cleaned up):")
        print(f"  {tmppath}")
        
        # Ask user if they want to copy files
        print("\nPress Enter to exit (files will be deleted)...")
        print("Or Ctrl+C to keep the temp directory for inspection")
        
        try:
            input()
        except KeyboardInterrupt:
            print(f"\n\nTemp directory preserved at: {tmppath}")
            print("Remember to delete it manually when done!")
            return True
        
        return True


def main():
    """Run the demo."""
    try:
        success = demo_conversion()
        if success:
            print("\n✓ Demo completed successfully!")
            sys.exit(0)
        else:
            print("\n✗ Demo encountered errors")
            sys.exit(1)
    except KeyboardInterrupt:
        print("\n\nDemo interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n✗ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""
YAML to WiX XML Converter
Converts YAML installer definitions to WiX XML format.

Handles the inline attribute strategy from the XSD converter.

Usage:
    python yaml_to_wix.py installer.yaml output.wxs
    python yaml_to_wix.py --input-dir installers --output-dir output
"""

import sys
import yaml
import argparse
from pathlib import Path
from typing import Any, Dict, List, Union
from xml.etree.ElementTree import Element, SubElement, tostring, ElementTree
from xml.dom import minidom


class YAMLToWiXConverter:
    """Converts YAML installer definitions to WiX XML."""
    
    # Properties that should be XML attributes, not child elements
    ATTRIBUTE_PROPERTIES = {
        'Id', 'Name', 'Version', 'Language', 'Manufacturer', 'UpgradeCode',
        'InstallerVersion', 'Compressed', 'InstallScope', 'Description', 'Comments',
        'DowngradeErrorMessage', 'Schedule', 'EmbedCab', 'CompressionLevel',
        'Guid', 'Win64', 'Source', 'KeyPath', 'Target', 'WorkingDirectory',
        'Icon', 'IconIndex', 'Root', 'Key', 'Type', 'Value', 'Title',
        'Level', 'ConfigurableDirectory', 'SourceFile', 'Arguments', 'On',
        'Directory', 'xmlns'
    }
    
    # Properties that contain text content (not attributes or child elements)
    TEXT_PROPERTIES = {'_value', 'value', 'text'}
    
    def __init__(self, attribute_strategy='inline'):
        """
        Initialize converter.
        
        Args:
            attribute_strategy: Must match the strategy used in XSD conversion
                - 'inline': Attributes are regular properties (default)
                - 'prefix': Attributes have @ prefix
                - 'separate': Attributes in 'attributes' object
        """
        self.attribute_strategy = attribute_strategy
    
    def convert(self, yaml_data: Dict[str, Any]) -> Element:
        """Convert YAML data to XML element tree."""
        # Handle root element
        if 'Wix' in yaml_data:
            root = self._dict_to_xml('Wix', yaml_data['Wix'])
        else:
            # If no Wix root, create one
            root = Element('Wix')
            for key, value in yaml_data.items():
                child = self._dict_to_xml(key, value)
                root.append(child)
        
        return root
    
    def _dict_to_xml(self, tag_name: str, data: Any, parent: Element = None) -> Element:
        """Recursively convert dictionary to XML element."""
        # Create element
        element = Element(tag_name) if parent is None else SubElement(parent, tag_name)
        
        # Handle different data types
        if isinstance(data, dict):
            self._process_dict(element, data)
        elif isinstance(data, list):
            # For lists, we need to create multiple elements with same tag
            # This happens when YAML has multiple items
            # Return just the first one, others will be handled by caller
            if data:
                self._process_dict(element, data[0] if isinstance(data[0], dict) else {})
        elif data is not None:
            # Simple value - set as text
            element.text = str(data)
        
        return element
    
    def _process_dict(self, element: Element, data: Dict[str, Any]):
        """Process dictionary and populate XML element."""
        attributes = {}
        children = {}
        text_content = None
        
        # Separate attributes from child elements based on strategy
        for key, value in data.items():
            if self.attribute_strategy == 'prefix':
                if key.startswith('@'):
                    # Attribute with @ prefix
                    attr_name = key[1:]  # Remove @
                    attributes[attr_name] = value
                elif key in self.TEXT_PROPERTIES:
                    text_content = value
                else:
                    children[key] = value
            
            elif self.attribute_strategy == 'separate':
                if key == 'attributes':
                    attributes.update(value)
                elif key == 'elements' or key == 'children':
                    children.update(value)
                elif key in self.TEXT_PROPERTIES:
                    text_content = value
                else:
                    # Default to child element
                    children[key] = value
            
            else:  # inline strategy (default)
                if key in self.ATTRIBUTE_PROPERTIES:
                    # This should be an attribute
                    attributes[key] = value
                elif key in self.TEXT_PROPERTIES:
                    text_content = value
                else:
                    # This is a child element
                    children[key] = value
        
        # Set attributes
        for attr_name, attr_value in attributes.items():
            element.set(attr_name, str(attr_value))
        
        # Set text content
        if text_content is not None:
            element.text = str(text_content)
        
        # Add child elements
        for child_name, child_value in children.items():
            if isinstance(child_value, list):
                # Multiple elements with same tag
                for item in child_value:
                    self._dict_to_xml(child_name, item, element)
            else:
                # Single child element
                self._dict_to_xml(child_name, child_value, element)
    
    def prettify(self, elem: Element) -> str:
        """Return a pretty-printed XML string."""
        rough_string = tostring(elem, encoding='unicode')
        reparsed = minidom.parseString(rough_string)
        return reparsed.toprettyxml(indent="  ")
    
    def save_xml(self, root: Element, output_path: str):
        """Save XML tree to file with proper formatting."""
        xml_string = self.prettify(root)
        
        # Remove extra blank lines
        lines = [line for line in xml_string.split('\n') if line.strip()]
        
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write('\n'.join(lines))


def load_yaml(yaml_path: str) -> dict:
    """Load YAML file."""
    with open(yaml_path, 'r', encoding='utf-8') as f:
        return yaml.safe_load(f)


def convert_file(input_path: str, output_path: str, attribute_strategy: str = 'inline'):
    """Convert a single YAML file to WiX XML."""
    print(f"Converting {input_path} -> {output_path}")
    
    try:
        # Load YAML
        yaml_data = load_yaml(input_path)
        
        # Convert to XML
        converter = YAMLToWiXConverter(attribute_strategy)
        xml_root = converter.convert(yaml_data)
        
        # Save XML
        converter.save_xml(xml_root, output_path)
        
        print(f"✓ Successfully converted to {output_path}")
        return True
        
    except Exception as e:
        print(f"✗ Error converting {input_path}: {e}")
        import traceback
        traceback.print_exc()
        return False


def convert_directory(input_dir: str, output_dir: str, attribute_strategy: str = 'inline'):
    """Convert all YAML files in a directory."""
    input_path = Path(input_dir)
    output_path = Path(output_dir)
    
    # Create output directory
    output_path.mkdir(parents=True, exist_ok=True)
    
    # Find all YAML files
    yaml_files = list(input_path.glob('*.yaml')) + list(input_path.glob('*.yml'))
    
    if not yaml_files:
        print(f"No YAML files found in {input_dir}")
        return
    
    print(f"Found {len(yaml_files)} YAML files\n")
    
    success_count = 0
    for yaml_file in yaml_files:
        # Generate output filename
        output_file = output_path / f"{yaml_file.stem}.wxs"
        
        if convert_file(str(yaml_file), str(output_file), attribute_strategy):
            success_count += 1
        print()
    
    print(f"\nConversion complete: {success_count}/{len(yaml_files)} successful")


def main():
    parser = argparse.ArgumentParser(
        description='Convert YAML installer definitions to WiX XML',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Convert single file
  python yaml_to_wix.py installer.yaml output.wxs
  
  # Convert directory
  python yaml_to_wix.py --input-dir installers --output-dir output
  
  # Convert with attribute prefix strategy
  python yaml_to_wix.py installer.yaml output.wxs --attribute-strategy prefix
        """
    )
    
    parser.add_argument('input', nargs='?', help='Input YAML file')
    parser.add_argument('output', nargs='?', help='Output WiX XML file')
    parser.add_argument('--input-dir', help='Input directory containing YAML files')
    parser.add_argument('--output-dir', help='Output directory for WiX XML files')
    parser.add_argument('--attribute-strategy',
                       choices=['inline', 'prefix', 'separate'],
                       default='inline',
                       help='Attribute handling strategy (must match XSD conversion)')
    
    args = parser.parse_args()
    
    # Directory mode
    if args.input_dir and args.output_dir:
        convert_directory(args.input_dir, args.output_dir, args.attribute_strategy)
        return
    
    # Single file mode
    if not args.input or not args.output:
        parser.error("Either provide input and output files, or use --input-dir and --output-dir")
    
    success = convert_file(args.input, args.output, args.attribute_strategy)
    sys.exit(0 if success else 1)


if __name__ == '__main__':
    main()

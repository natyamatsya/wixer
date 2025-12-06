#!/usr/bin/env python3
"""
XSD to JSON Schema Converter
Converts XML Schema Definition (XSD) files to JSON Schema for YAML validation.

Handles WiX v4 schemas with support for:
- Complex and simple types
- Elements and attributes
- Sequences, choices, and all groups
- Restrictions and enumerations
- Multiple namespaces
- xs:include and xs:import

Usage:
    python xsd_to_jsonschema.py [options]
"""

import os
import sys
import json
import argparse
import urllib.request
from pathlib import Path
from lxml import etree
from collections import defaultdict, OrderedDict
from typing import Dict, List, Any, Optional, Set, Tuple
import re


class XSDToJSONSchemaConverter:
    """Converts XSD schemas to JSON Schema format."""
    
    XS_NAMESPACE = "http://www.w3.org/2001/XMLSchema"
    
    # XSD to JSON Schema type mapping
    TYPE_MAPPING = {
        'string': 'string',
        'normalizedString': 'string',
        'token': 'string',
        'NCName': 'string',
        'NMTOKEN': 'string',
        'anyURI': 'string',
        'language': 'string',
        'ID': 'string',
        'IDREF': 'string',
        
        'boolean': 'boolean',
        
        'integer': 'integer',
        'int': 'integer',
        'long': 'integer',
        'short': 'integer',
        'byte': 'integer',
        'nonNegativeInteger': 'integer',
        'positiveInteger': 'integer',
        'unsignedLong': 'integer',
        'unsignedInt': 'integer',
        'unsignedShort': 'integer',
        'unsignedByte': 'integer',
        
        'decimal': 'number',
        'float': 'number',
        'double': 'number',
    }
    
    def __init__(self, attribute_strategy='inline', include_descriptions=True, 
                 namespace_prefix=True, strict_mode=False):
        """
        Initialize converter with options.
        
        Args:
            attribute_strategy: How to handle XML attributes
                - 'inline': Attributes become regular properties (default)
                - 'prefix': Attributes get '@' prefix (@id, @name, etc.)
                - 'separate': Attributes in separate 'attributes' object
            include_descriptions: Include XSD annotations as descriptions
            namespace_prefix: Include namespace prefixes in element names
            strict_mode: More strict validation (required fields, etc.)
        """
        self.attribute_strategy = attribute_strategy
        self.include_descriptions = include_descriptions
        self.namespace_prefix = namespace_prefix
        self.strict_mode = strict_mode
        
        self.schemas = {}  # namespace -> schema tree
        self.types = {}    # qualified name -> type definition
        self.elements = {}  # qualified name -> element definition
        self.namespaces = {}  # prefix -> uri
        self.target_namespace = None
        
    def load_schema(self, path_or_url: str) -> etree._Element:
        """Load XSD schema from file or URL."""
        print(f"Loading schema: {path_or_url}")
        
        if path_or_url.startswith('http://') or path_or_url.startswith('https://'):
            with urllib.request.urlopen(path_or_url) as response:
                content = response.read()
            tree = etree.fromstring(content)
        else:
            tree = etree.parse(path_or_url).getroot()
            
        return tree
    
    def parse_schema(self, schema_tree: etree._Element, base_path: Optional[str] = None):
        """Parse an XSD schema and extract all definitions."""
        # Get target namespace
        self.target_namespace = schema_tree.get('targetNamespace')
        
        # Extract namespace prefixes
        for prefix, uri in schema_tree.nsmap.items():
            if prefix is not None:
                self.namespaces[prefix] = uri
        
        # Store schema
        if self.target_namespace:
            self.schemas[self.target_namespace] = schema_tree
        
        # Process includes and imports first
        self._process_includes(schema_tree, base_path)
        
        # Extract all type definitions
        for complex_type in schema_tree.findall(f'.//{{{self.XS_NAMESPACE}}}complexType'):
            name = complex_type.get('name')
            if name:
                qualified_name = self._qualify_name(name)
                self.types[qualified_name] = complex_type
        
        for simple_type in schema_tree.findall(f'.//{{{self.XS_NAMESPACE}}}simpleType'):
            name = simple_type.get('name')
            if name:
                qualified_name = self._qualify_name(name)
                self.types[qualified_name] = simple_type
        
        # Extract top-level elements
        for element in schema_tree.findall(f'{{{self.XS_NAMESPACE}}}element'):
            name = element.get('name')
            if name:
                qualified_name = self._qualify_name(name)
                self.elements[qualified_name] = element
    
    def _process_includes(self, schema_tree: etree._Element, base_path: Optional[str]):
        """Process xs:include and xs:import directives."""
        for include in schema_tree.findall(f'{{{self.XS_NAMESPACE}}}include'):
            schema_location = include.get('schemaLocation')
            if schema_location and base_path:
                included_path = os.path.join(os.path.dirname(base_path), schema_location)
                if os.path.exists(included_path):
                    included_tree = self.load_schema(included_path)
                    self.parse_schema(included_tree, included_path)
        
        for import_elem in schema_tree.findall(f'{{{self.XS_NAMESPACE}}}import'):
            schema_location = import_elem.get('schemaLocation')
            namespace = import_elem.get('namespace')
            if schema_location and base_path:
                imported_path = os.path.join(os.path.dirname(base_path), schema_location)
                if os.path.exists(imported_path):
                    imported_tree = self.load_schema(imported_path)
                    # Temporarily switch namespace context
                    old_ns = self.target_namespace
                    self.parse_schema(imported_tree, imported_path)
                    self.target_namespace = old_ns
    
    def _qualify_name(self, name: str, namespace: Optional[str] = None) -> str:
        """Create qualified name with namespace."""
        ns = namespace or self.target_namespace
        if ns:
            return f"{{{ns}}}{name}"
        return name
    
    def _get_annotation(self, element: etree._Element) -> Optional[str]:
        """Extract documentation from xs:annotation."""
        if not self.include_descriptions:
            return None
        
        annotation = element.find(f'{{{self.XS_NAMESPACE}}}annotation')
        if annotation is not None:
            docs = []
            for doc in annotation.findall(f'{{{self.XS_NAMESPACE}}}documentation'):
                if doc.text:
                    docs.append(doc.text.strip())
            return ' '.join(docs) if docs else None
        return None
    
    def convert_type(self, type_name: str) -> Dict[str, Any]:
        """Convert XSD type to JSON Schema type."""
        # Handle built-in XSD types
        if ':' in type_name:
            prefix, local_name = type_name.split(':', 1)
            if prefix == 'xs' or prefix == 'xsd':
                json_type = self.TYPE_MAPPING.get(local_name, 'string')
                return {'type': json_type}
        
        # Handle qualified names
        qualified_name = self._qualify_name(type_name)
        
        # Look up custom type
        if qualified_name in self.types:
            return self._convert_type_definition(self.types[qualified_name])
        
        # Default to string if unknown
        return {'type': 'string'}
    
    def _convert_type_definition(self, type_elem: etree._Element) -> Dict[str, Any]:
        """Convert a complexType or simpleType definition."""
        schema = {}
        
        # Get documentation
        description = self._get_annotation(type_elem)
        if description:
            schema['description'] = description
        
        # Handle simpleType
        if type_elem.tag == f'{{{self.XS_NAMESPACE}}}simpleType':
            return self._convert_simple_type(type_elem)
        
        # Handle complexType
        if type_elem.tag == f'{{{self.XS_NAMESPACE}}}complexType':
            return self._convert_complex_type(type_elem)
        
        return schema
    
    def _convert_simple_type(self, simple_type: etree._Element) -> Dict[str, Any]:
        """Convert simpleType with restrictions."""
        schema = {}
        
        # Check for restriction
        restriction = simple_type.find(f'{{{self.XS_NAMESPACE}}}restriction')
        if restriction is not None:
            base = restriction.get('base')
            if base:
                schema.update(self.convert_type(base))
            
            # Handle enumerations
            enums = restriction.findall(f'{{{self.XS_NAMESPACE}}}enumeration')
            if enums:
                schema['enum'] = [e.get('value') for e in enums]
            
            # Handle patterns
            pattern = restriction.find(f'{{{self.XS_NAMESPACE}}}pattern')
            if pattern is not None:
                schema['pattern'] = pattern.get('value')
            
            # Handle length constraints
            min_length = restriction.find(f'{{{self.XS_NAMESPACE}}}minLength')
            if min_length is not None:
                schema['minLength'] = int(min_length.get('value'))
            
            max_length = restriction.find(f'{{{self.XS_NAMESPACE}}}maxLength')
            if max_length is not None:
                schema['maxLength'] = int(max_length.get('value'))
            
            # Handle numeric constraints
            min_inclusive = restriction.find(f'{{{self.XS_NAMESPACE}}}minInclusive')
            if min_inclusive is not None:
                schema['minimum'] = float(min_inclusive.get('value'))
            
            max_inclusive = restriction.find(f'{{{self.XS_NAMESPACE}}}maxInclusive')
            if max_inclusive is not None:
                schema['maximum'] = float(max_inclusive.get('value'))
        
        return schema
    
    def _convert_complex_type(self, complex_type: etree._Element) -> Dict[str, Any]:
        """Convert complexType to JSON Schema object."""
        schema = {
            'type': 'object',
            'properties': {},
            'additionalProperties': False
        }
        
        required = []
        
        # Handle attributes
        for attr in complex_type.findall(f'.//{{{self.XS_NAMESPACE}}}attribute'):
            attr_name = attr.get('name')
            if not attr_name:
                continue
            
            # Determine property name based on strategy
            if self.attribute_strategy == 'prefix':
                prop_name = f'@{attr_name}'
            else:
                prop_name = attr_name
            
            attr_schema = self._convert_attribute(attr)
            schema['properties'][prop_name] = attr_schema
            
            # Check if required
            use = attr.get('use', 'optional')
            if use == 'required' and self.strict_mode:
                required.append(prop_name)
        
        # Handle elements (sequence, choice, all)
        self._process_content_model(complex_type, schema, required)
        
        # Handle simple content
        simple_content = complex_type.find(f'{{{self.XS_NAMESPACE}}}simpleContent')
        if simple_content is not None:
            extension = simple_content.find(f'{{{self.XS_NAMESPACE}}}extension')
            if extension is not None:
                base = extension.get('base')
                if base:
                    # Add value property for text content
                    base_schema = self.convert_type(base)
                    if self.attribute_strategy == 'inline':
                        schema['properties']['_value'] = base_schema
                    else:
                        schema['properties']['value'] = base_schema
        
        # Handle complex content
        complex_content = complex_type.find(f'{{{self.XS_NAMESPACE}}}complexContent')
        if complex_content is not None:
            extension = complex_content.find(f'{{{self.XS_NAMESPACE}}}extension')
            restriction = complex_content.find(f'{{{self.XS_NAMESPACE}}}restriction')
            
            base_elem = extension if extension is not None else restriction
            if base_elem is not None:
                base = base_elem.get('base')
                if base:
                    # Merge base type properties
                    base_schema = self.convert_type(base)
                    if 'properties' in base_schema:
                        schema['properties'].update(base_schema['properties'])
                    if 'required' in base_schema:
                        required.extend(base_schema['required'])
                
                # Process additional elements/attributes in extension
                self._process_content_model(base_elem, schema, required)
        
        if required:
            schema['required'] = list(set(required))
        
        return schema
    
    def _convert_attribute(self, attr: etree._Element) -> Dict[str, Any]:
        """Convert attribute to JSON Schema property."""
        schema = {}
        
        # Get type
        type_name = attr.get('type')
        if type_name:
            schema.update(self.convert_type(type_name))
        else:
            # Inline simple type
            simple_type = attr.find(f'{{{self.XS_NAMESPACE}}}simpleType')
            if simple_type is not None:
                schema.update(self._convert_simple_type(simple_type))
            else:
                schema['type'] = 'string'
        
        # Get documentation
        description = self._get_annotation(attr)
        if description:
            schema['description'] = description
        
        # Get default value
        default = attr.get('default')
        if default:
            schema['default'] = default
        
        return schema
    
    def _process_content_model(self, parent: etree._Element, 
                               schema: Dict[str, Any], required: List[str]):
        """Process sequence, choice, or all groups."""
        # Process sequence
        for sequence in parent.findall(f'.//{{{self.XS_NAMESPACE}}}sequence'):
            for element in sequence.findall(f'{{{self.XS_NAMESPACE}}}element'):
                self._process_element(element, schema, required)
        
        # Process choice
        for choice in parent.findall(f'.//{{{self.XS_NAMESPACE}}}choice'):
            # For choices, we create oneOf
            choice_schemas = []
            for element in choice.findall(f'{{{self.XS_NAMESPACE}}}element'):
                elem_schema = self._convert_element(element)
                elem_name = element.get('name') or element.get('ref')
                if elem_name:
                    choice_schemas.append({
                        'type': 'object',
                        'properties': {elem_name: elem_schema},
                        'required': [elem_name]
                    })
            
            if choice_schemas:
                schema['oneOf'] = choice_schemas
        
        # Process all
        for all_group in parent.findall(f'.//{{{self.XS_NAMESPACE}}}all'):
            for element in all_group.findall(f'{{{self.XS_NAMESPACE}}}element'):
                self._process_element(element, schema, required)
    
    def _process_element(self, element: etree._Element, 
                        schema: Dict[str, Any], required: List[str]):
        """Process an element and add to schema properties."""
        elem_name = element.get('name') or element.get('ref')
        if not elem_name:
            return
        
        elem_schema = self._convert_element(element)
        
        # Handle maxOccurs > 1 (arrays)
        max_occurs = element.get('maxOccurs', '1')
        min_occurs = element.get('minOccurs', '1')
        
        if max_occurs == 'unbounded' or (max_occurs.isdigit() and int(max_occurs) > 1):
            # This is an array
            array_schema = {
                'type': 'array',
                'items': elem_schema
            }
            if min_occurs.isdigit():
                array_schema['minItems'] = int(min_occurs)
            if max_occurs.isdigit():
                array_schema['maxItems'] = int(max_occurs)
            
            schema['properties'][elem_name] = array_schema
        else:
            schema['properties'][elem_name] = elem_schema
        
        # Check if required
        if min_occurs != '0' and self.strict_mode:
            required.append(elem_name)
    
    def _convert_element(self, element: etree._Element) -> Dict[str, Any]:
        """Convert an element definition to JSON Schema."""
        schema = {}
        
        # Get documentation
        description = self._get_annotation(element)
        if description:
            schema['description'] = description
        
        # Get type
        type_name = element.get('type')
        if type_name:
            schema.update(self.convert_type(type_name))
        else:
            # Inline type definition
            complex_type = element.find(f'{{{self.XS_NAMESPACE}}}complexType')
            simple_type = element.find(f'{{{self.XS_NAMESPACE}}}simpleType')
            
            if complex_type is not None:
                schema.update(self._convert_complex_type(complex_type))
            elif simple_type is not None:
                schema.update(self._convert_simple_type(simple_type))
            else:
                schema['type'] = 'string'
        
        return schema
    
    def generate_schema(self, root_element_name: Optional[str] = None) -> Dict[str, Any]:
        """Generate complete JSON Schema."""
        schema = {
            '$schema': 'https://json-schema.org/draft-07/schema#',
            'type': 'object',
            'definitions': {}
        }
        
        # Add all named types as definitions
        for qualified_name, type_elem in self.types.items():
            # Extract local name
            local_name = qualified_name.split('}')[-1] if '}' in qualified_name else qualified_name
            schema['definitions'][local_name] = self._convert_type_definition(type_elem)
        
        # If root element specified, make it the schema root
        if root_element_name:
            qualified_name = self._qualify_name(root_element_name)
            if qualified_name in self.elements:
                root_schema = self._convert_element(self.elements[qualified_name])
                schema.update(root_schema)
            else:
                print(f"Warning: Root element '{root_element_name}' not found")
        else:
            # Create properties for all top-level elements
            schema['properties'] = {}
            for qualified_name, element in self.elements.items():
                local_name = qualified_name.split('}')[-1] if '}' in qualified_name else qualified_name
                schema['properties'][local_name] = self._convert_element(element)
        
        return schema


def download_wix_schemas(output_dir: str = 'schemas') -> List[str]:
    """Download all WiX v4 schema files from GitHub."""
    os.makedirs(output_dir, exist_ok=True)
    
    base_url = 'https://raw.githubusercontent.com/wixtoolset/wix/develop/src/api/wix/'
    
    schemas = [
        'wix.xsd',
        'wixloc.xsd',
        # Extensions
        'bal/bal.xsd',
        'complus/complus.xsd',
        'dependency/dependency.xsd',
        'difxapp/difxapp.xsd',
        'firewall/firewall.xsd',
        'http/http.xsd',
        'iis/iis.xsd',
        'netfx/netfx.xsd',
        'ps/ps.xsd',
        'sql/sql.xsd',
        'util/util.xsd',
        'vs/vs.xsd',
    ]
    
    downloaded = []
    for schema_path in schemas:
        url = base_url + schema_path
        filename = schema_path.replace('/', '_')
        output_path = os.path.join(output_dir, filename)
        
        try:
            print(f"Downloading {schema_path}...")
            urllib.request.urlretrieve(url, output_path)
            downloaded.append(output_path)
        except Exception as e:
            print(f"Failed to download {schema_path}: {e}")
    
    return downloaded


def main():
    parser = argparse.ArgumentParser(
        description='Convert XSD schemas to JSON Schema for YAML validation',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Download and convert all WiX schemas
  python xsd_to_jsonschema.py --download-wix --output wix-schemas
  
  # Convert a single XSD file
  python xsd_to_jsonschema.py -i schema.xsd -o schema.json
  
  # Convert with attribute prefix strategy
  python xsd_to_jsonschema.py -i schema.xsd --attribute-strategy prefix
  
  # Convert with strict validation
  python xsd_to_jsonschema.py -i schema.xsd --strict
        """
    )
    
    parser.add_argument('-i', '--input', help='Input XSD file path or URL')
    parser.add_argument('-o', '--output', help='Output JSON Schema file')
    parser.add_argument('--download-wix', action='store_true',
                       help='Download all WiX v4 schemas from GitHub')
    parser.add_argument('--schema-dir', default='schemas',
                       help='Directory for downloaded schemas (default: schemas)')
    parser.add_argument('--output-dir', default='json-schemas',
                       help='Directory for generated JSON schemas (default: json-schemas)')
    parser.add_argument('--attribute-strategy', 
                       choices=['inline', 'prefix', 'separate'],
                       default='inline',
                       help='How to handle XML attributes (default: inline)')
    parser.add_argument('--no-descriptions', action='store_true',
                       help='Exclude descriptions from schemas')
    parser.add_argument('--strict', action='store_true',
                       help='Enable strict mode (enforce required fields)')
    parser.add_argument('--root-element', 
                       help='Root element name for schema')
    
    args = parser.parse_args()
    
    # Download WiX schemas if requested
    if args.download_wix:
        print("Downloading WiX v4 schemas...")
        schema_files = download_wix_schemas(args.schema_dir)
        print(f"\nDownloaded {len(schema_files)} schema files\n")
    elif args.input:
        schema_files = [args.input]
    else:
        parser.error("Either --input or --download-wix is required")
        return
    
    # Create output directory
    os.makedirs(args.output_dir, exist_ok=True)
    
    # Convert each schema
    for schema_file in schema_files:
        print(f"\nConverting {schema_file}...")
        
        try:
            # Initialize converter
            converter = XSDToJSONSchemaConverter(
                attribute_strategy=args.attribute_strategy,
                include_descriptions=not args.no_descriptions,
                strict_mode=args.strict
            )
            
            # Load and parse schema
            schema_tree = converter.load_schema(schema_file)
            converter.parse_schema(schema_tree, schema_file)
            
            # Generate JSON Schema
            json_schema = converter.generate_schema(args.root_element)
            
            # Determine output filename
            if args.output:
                output_file = args.output
            else:
                basename = os.path.basename(schema_file)
                name = os.path.splitext(basename)[0]
                output_file = os.path.join(args.output_dir, f'{name}.schema.json')
            
            # Write output
            with open(output_file, 'w', encoding='utf-8') as f:
                json.dump(json_schema, f, indent=2, ensure_ascii=False)
            
            print(f"✓ Generated: {output_file}")
            
            # Print statistics
            num_defs = len(json_schema.get('definitions', {}))
            num_props = len(json_schema.get('properties', {}))
            print(f"  - {num_defs} type definitions")
            print(f"  - {num_props} root properties")
            
        except Exception as e:
            print(f"✗ Error converting {schema_file}: {e}")
            import traceback
            traceback.print_exc()
    
    print(f"\nConversion complete! JSON schemas saved to {args.output_dir}/")
    print(f"\nNext steps:")
    print(f"1. Review the generated schemas in {args.output_dir}/")
    print(f"2. Create YAML files conforming to these schemas")
    print(f"3. Validate YAML with: check-jsonschema --schemafile schema.json file.yaml")


if __name__ == '__main__':
    main()

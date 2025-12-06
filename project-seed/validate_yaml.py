#!/usr/bin/env python3
"""
YAML Validator
Validates YAML files against JSON Schema generated from XSD.

Usage:
    python validate_yaml.py schema.json file.yaml
    python validate_yaml.py --schema-dir json-schemas --yaml-dir installers
"""

import sys
import json
import yaml
import argparse
from pathlib import Path
from typing import List, Tuple
import jsonschema
from jsonschema import validate, ValidationError, Draft7Validator


def load_yaml(yaml_path: str) -> dict:
    """Load YAML file."""
    with open(yaml_path, 'r', encoding='utf-8') as f:
        return yaml.safe_load(f)


def load_schema(schema_path: str) -> dict:
    """Load JSON Schema file."""
    with open(schema_path, 'r', encoding='utf-8') as f:
        return json.load(f)


def validate_yaml_file(yaml_path: str, schema_path: str) -> Tuple[bool, List[str]]:
    """
    Validate a YAML file against a JSON Schema.
    
    Returns:
        Tuple of (is_valid, list_of_errors)
    """
    try:
        # Load files
        data = load_yaml(yaml_path)
        schema = load_schema(schema_path)
        
        # Validate
        validator = Draft7Validator(schema)
        errors = list(validator.iter_errors(data))
        
        if not errors:
            return True, []
        
        # Format errors
        error_messages = []
        for error in errors:
            path = '.'.join(str(p) for p in error.path) if error.path else 'root'
            error_messages.append(f"  • {path}: {error.message}")
        
        return False, error_messages
        
    except yaml.YAMLError as e:
        return False, [f"YAML parsing error: {e}"]
    except json.JSONDecodeError as e:
        return False, [f"Schema parsing error: {e}"]
    except Exception as e:
        return False, [f"Validation error: {e}"]


def validate_directory(schema_dir: str, yaml_dir: str, schema_suffix: str = '.schema.json'):
    """Validate all YAML files in a directory."""
    schema_path = Path(schema_dir)
    yaml_path = Path(yaml_dir)
    
    # Find all YAML files
    yaml_files = list(yaml_path.glob('**/*.yaml')) + list(yaml_path.glob('**/*.yml'))
    
    if not yaml_files:
        print(f"No YAML files found in {yaml_dir}")
        return
    
    print(f"Found {len(yaml_files)} YAML files\n")
    
    results = []
    for yaml_file in yaml_files:
        # Try to find matching schema
        # Look for schema with same base name
        base_name = yaml_file.stem
        schema_file = schema_path / f"{base_name}{schema_suffix}"
        
        if not schema_file.exists():
            # Try common schema names
            for common in ['wix', 'installer', 'package']:
                schema_file = schema_path / f"{common}{schema_suffix}"
                if schema_file.exists():
                    break
        
        if not schema_file.exists():
            print(f"⚠ {yaml_file.name}: No matching schema found")
            results.append(('skip', yaml_file.name, None))
            continue
        
        # Validate
        print(f"Validating {yaml_file.name} against {schema_file.name}...")
        is_valid, errors = validate_yaml_file(str(yaml_file), str(schema_file))
        
        if is_valid:
            print(f"✓ {yaml_file.name}: Valid\n")
            results.append(('pass', yaml_file.name, None))
        else:
            print(f"✗ {yaml_file.name}: Invalid")
            for error in errors:
                print(error)
            print()
            results.append(('fail', yaml_file.name, errors))
    
    # Print summary
    print("\n" + "="*60)
    print("VALIDATION SUMMARY")
    print("="*60)
    
    passed = sum(1 for r in results if r[0] == 'pass')
    failed = sum(1 for r in results if r[0] == 'fail')
    skipped = sum(1 for r in results if r[0] == 'skip')
    
    print(f"✓ Passed: {passed}")
    print(f"✗ Failed: {failed}")
    print(f"⚠ Skipped: {skipped}")
    print(f"Total: {len(results)}")
    
    return failed == 0


def main():
    parser = argparse.ArgumentParser(
        description='Validate YAML files against JSON Schema',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Validate single file
  python validate_yaml.py schema.json installer.yaml
  
  # Validate directory
  python validate_yaml.py --schema-dir json-schemas --yaml-dir installers
  
  # Show detailed validation info
  python validate_yaml.py -v schema.json installer.yaml
        """
    )
    
    parser.add_argument('schema', nargs='?', help='JSON Schema file path')
    parser.add_argument('yaml_file', nargs='?', help='YAML file to validate')
    parser.add_argument('--schema-dir', help='Directory containing JSON schemas')
    parser.add_argument('--yaml-dir', help='Directory containing YAML files')
    parser.add_argument('-v', '--verbose', action='store_true',
                       help='Verbose output')
    
    args = parser.parse_args()
    
    # Directory mode
    if args.schema_dir and args.yaml_dir:
        success = validate_directory(args.schema_dir, args.yaml_dir)
        sys.exit(0 if success else 1)
    
    # Single file mode
    if not args.schema or not args.yaml_file:
        parser.error("Either provide schema and yaml_file, or use --schema-dir and --yaml-dir")
    
    print(f"Validating {args.yaml_file} against {args.schema}...\n")
    
    is_valid, errors = validate_yaml_file(args.yaml_file, args.schema)
    
    if is_valid:
        print("✓ YAML is valid!")
        sys.exit(0)
    else:
        print("✗ YAML is invalid:\n")
        for error in errors:
            print(error)
        sys.exit(1)


if __name__ == '__main__':
    main()

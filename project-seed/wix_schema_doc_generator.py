#!/usr/bin/env python3
"""
WiX Schema Documentation Generator
Generates readable HTML and Markdown documentation from WiX v4 XSD schema files.
Includes Mermaid diagram generation for visualizing schema relationships.
"""

import os
import sys
import urllib.request
from pathlib import Path
from lxml import etree
from collections import defaultdict
import html
import argparse

# XSD namespace
XS_NS = "http://www.w3.org/2001/XMLSchema"
NSMAP = {"xs": XS_NS}

# WiX v4 schema URLs
SCHEMA_BASE_URL = "https://raw.githubusercontent.com/wixtoolset/web/master/src/xsd4/"
SCHEMAS = [
    "wix.xsd",
    "wixloc.xsd",
    "bal.xsd",
    "complus.xsd",
    "dependency.xsd",
    "difxapp.xsd",
    "firewall.xsd",
    "http.xsd",
    "iis.xsd",
    "netfx.xsd",
    "ps.xsd",
    "sql.xsd",
    "util.xsd",
    "vs.xsd",
]


class SchemaDocGenerator:
    def __init__(self, output_dir="wix_docs", format="html"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(exist_ok=True)
        self.format = format  # "html", "markdown", "asciidoc", "all", or combinations
        self.schemas = {}
        self.elements = defaultdict(list)  # element_name -> [(schema, element_node)]
        self.types = defaultdict(list)  # type_name -> [(schema, type_node)]
        self.relationships = defaultdict(list)  # parent -> [children]
        
    def download_schemas(self):
        """Download all schema files from GitHub."""
        print("Downloading WiX v4 schemas...")
        schema_dir = self.output_dir / "schemas"
        schema_dir.mkdir(exist_ok=True)
        
        for schema in SCHEMAS:
            url = SCHEMA_BASE_URL + schema
            output_path = schema_dir / schema
            
            try:
                print(f"  Downloading {schema}...")
                urllib.request.urlretrieve(url, output_path)
            except Exception as e:
                print(f"  Warning: Could not download {schema}: {e}")
                
        return schema_dir
    
    def parse_schemas(self, schema_dir):
        """Parse all XSD files."""
        print("\nParsing schemas...")
        
        for schema_file in schema_dir.glob("*.xsd"):
            try:
                print(f"  Parsing {schema_file.name}...")
                tree = etree.parse(str(schema_file))
                self.schemas[schema_file.stem] = tree
                
                # Index elements
                for elem in tree.xpath("//xs:element[@name]", namespaces=NSMAP):
                    name = elem.get("name")
                    self.elements[name].append((schema_file.stem, elem))
                
                # Index types
                for type_elem in tree.xpath("//xs:complexType[@name] | //xs:simpleType[@name]", 
                                           namespaces=NSMAP):
                    name = type_elem.get("name")
                    self.types[name].append((schema_file.stem, type_elem))
                    
            except Exception as e:
                print(f"  Error parsing {schema_file.name}: {e}")
        
        # Build relationships
        self._build_relationships()
    
    def extract_documentation(self, element):
        """Extract documentation text from xs:documentation elements directly on this element (not from attributes/children)."""
        # Only get documentation from direct child xs:annotation, not from nested attributes or child elements
        # We need to find xs:annotation that's a direct child of this element
        annotations = []
        for child in element:
            if child.tag == f"{{{XS_NS}}}annotation":
                annotations.append(child)
        
        if not annotations:
            return None
        
        text_parts = []
        for annotation in annotations:
            # Get documentation only from this annotation, not nested ones
            for child in annotation:
                if child.tag == f"{{{XS_NS}}}documentation":
                    text = self._extract_text_recursive(child)
                    if text:
                        text_parts.append(text.strip())
        
        return "\n\n".join(text_parts) if text_parts else None
    
    def _extract_text_recursive(self, element):
        """Recursively extract text content, handling mixed content."""
        parts = []
        if element.text:
            parts.append(element.text)
        
        for child in element:
            # Handle child elements
            if child.tag.endswith("p"):
                parts.append(self._extract_text_recursive(child))
            elif child.tag.endswith("a"):
                link_text = self._extract_text_recursive(child)
                href = child.get("href", "")
                parts.append(f'<a href="{href}">{link_text}</a>')
            else:
                parts.append(self._extract_text_recursive(child))
            
            if child.tail:
                parts.append(child.tail)
        
        return "".join(parts)
    
    def clean_html_for_typst(self, text):
        """Convert HTML markup to Typst format and escape special characters."""
        if not text:
            return ""
        
        import re
        
        # Convert <a href="url">text</a> to Typst link format
        text = re.sub(r'<a\s+href="([^"]+)">([^<]+)</a>', r'#link("\1")[\2]', text, flags=re.IGNORECASE)
        
        # Remove any remaining HTML tags but keep their content
        text = re.sub(r'<br\s*/?>', ' ', text, flags=re.IGNORECASE)
        text = re.sub(r'<p>', '\n\n', text, flags=re.IGNORECASE)
        text = re.sub(r'</p>', '', text, flags=re.IGNORECASE)
        text = re.sub(r'<[^>]+>', '', text)
        
        # Clean up HTML entities
        text = text.replace('&lt;', '<')
        text = text.replace('&gt;', '>')
        text = text.replace('&amp;', '&')
        text = text.replace('&quot;', '"')
        text = text.replace('&#39;', "'")
        
        # Escape special Typst characters (but not inside #link constructs)
        # We need to be careful not to escape characters inside #link() calls
        
        # Split by #link to preserve them
        parts = re.split(r'(#link\("[^"]*"\)\[[^\]]*\])', text)
        
        escaped_parts = []
        for i, part in enumerate(parts):
            if i % 2 == 0:  # Not a #link construct
                # Escape special Typst characters in the correct order
                part = part.replace('\\', '\\\\')  # Backslash first (it's the escape char)
                part = part.replace('`', '\\`')    # Backtick (raw text/code)
                part = part.replace('*', '\\*')    # Asterisk (emphasis)
                part = part.replace('_', '\\_')    # Underscore (subscript)
                part = part.replace('#', '\\#')    # Hash (function call)
                part = part.replace('@', '\\@')    # At sign (label reference)
                part = part.replace('[', '\\[')    # Opening bracket
                part = part.replace(']', '\\]')    # Closing bracket
                part = part.replace('$', '\\$')    # Dollar sign (math mode)
            escaped_parts.append(part)
        
        return ''.join(escaped_parts)
    
    def get_attributes(self, element):
        """Get all attributes of an element."""
        attrs = []
        
        # Direct attributes
        for attr in element.xpath(".//xs:attribute", namespaces=NSMAP):
            attr_info = {
                "name": attr.get("name"),
                "type": attr.get("type", ""),
                "use": attr.get("use", "optional"),
                "doc": self.extract_documentation(attr)
            }
            attrs.append(attr_info)
        
        # Attribute groups
        for attr_group_ref in element.xpath(".//xs:attributeGroup[@ref]", namespaces=NSMAP):
            ref = attr_group_ref.get("ref")
            attrs.append({
                "name": f"[Group: {ref}]",
                "type": "",
                "use": "",
                "doc": f"References attribute group: {ref}"
            })
        
        return attrs
    
    def get_child_elements(self, element):
        """Get allowed child elements."""
        children = []
        
        # Look for element references in sequences, choices, etc.
        for child_elem in element.xpath(".//xs:element[@ref] | .//xs:element[@name]", 
                                       namespaces=NSMAP):
            ref = child_elem.get("ref") or child_elem.get("name")
            min_occurs = child_elem.get("minOccurs", "1")
            max_occurs = child_elem.get("maxOccurs", "1")
            
            children.append({
                "name": ref,
                "minOccurs": min_occurs,
                "maxOccurs": max_occurs,
                "doc": self.extract_documentation(child_elem)
            })
        
        return children
    
    def generate_element_page(self, element_name, schema_name, element_node):
        """Generate HTML page for a single element."""
        doc = self.extract_documentation(element_node)
        attrs = self.get_attributes(element_node)
        children = self.get_child_elements(element_node)
        
        # Get parent information from xs:appinfo
        parents = []
        for parent in element_node.xpath(".//xs:appinfo//xse:parent", namespaces={
            "xs": XS_NS,
            "xse": "http://wixtoolset.org/schemas/XmlSchemaExtension"
        }):
            parent_ref = parent.get("ref", "")
            parents.append(parent_ref)
        
        html_content = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{element_name} - WiX v4 Schema</title>
    <style>
        {self.get_css()}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>{html.escape(element_name)}</h1>
            <p class="schema-name">Schema: {html.escape(schema_name)}.xsd</p>
        </div>
        
        <nav class="breadcrumb">
            <a href="index.html">Index</a> &raquo; 
            <a href="schema_{html.escape(schema_name)}.html">{html.escape(schema_name)}</a> &raquo;
            {html.escape(element_name)}
        </nav>
        
        {f'<div class="description">{self._format_doc(doc)}</div>' if doc else ''}
        
        {self._generate_parents_section(parents) if parents else ''}
        {self._generate_attributes_section(attrs) if attrs else ''}
        {self._generate_children_section(children) if children else ''}
        
        <div class="footer">
            Generated from WiX v4 schemas
        </div>
    </div>
</body>
</html>"""
        
        output_file = self.output_dir / f"element_{element_name}.html"
        output_file.write_text(html_content, encoding="utf-8")
    
    def _format_doc(self, doc_text):
        """Format documentation text as HTML."""
        if not doc_text:
            return ""
        
        # Simple markdown-like processing
        lines = doc_text.split("\n")
        formatted = []
        
        for line in lines:
            line = line.strip()
            if not line:
                formatted.append("<br>")
            else:
                # Don't escape if it already contains HTML tags
                if "<a href" in line:
                    formatted.append(f"<p>{line}</p>")
                else:
                    formatted.append(f"<p>{html.escape(line)}</p>")
        
        return "\n".join(formatted)
    
    def _generate_parents_section(self, parents):
        """Generate HTML for parent elements section."""
        if not parents:
            return ""
        
        parent_links = [f'<a href="element_{html.escape(p)}.html">{html.escape(p)}</a>' 
                       for p in parents]
        
        return f"""
        <div class="section">
            <h2>Parent Elements</h2>
            <p>This element can be a child of: {", ".join(parent_links)}</p>
        </div>"""
    
    def _generate_attributes_section(self, attrs):
        """Generate HTML for attributes section."""
        if not attrs:
            return ""
        
        rows = []
        for attr in attrs:
            doc_cell = f'<td class="doc">{self._format_doc(attr["doc"])}</td>' if attr["doc"] else '<td></td>'
            use_class = "required" if attr["use"] == "required" else "optional"
            
            rows.append(f"""
            <tr>
                <td class="attr-name">{html.escape(attr["name"])}</td>
                <td class="attr-type">{html.escape(attr["type"])}</td>
                <td class="attr-use {use_class}">{html.escape(attr["use"])}</td>
                {doc_cell}
            </tr>""")
        
        return f"""
        <div class="section">
            <h2>Attributes</h2>
            <table class="attrs-table">
                <thead>
                    <tr>
                        <th>Name</th>
                        <th>Type</th>
                        <th>Use</th>
                        <th>Description</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(rows)}
                </tbody>
            </table>
        </div>"""
    
    def _generate_children_section(self, children):
        """Generate HTML for child elements section."""
        if not children:
            return ""
        
        rows = []
        for child in children:
            cardinality = f"{child['minOccurs']}..{child['maxOccurs']}"
            doc_cell = f'<td class="doc">{self._format_doc(child["doc"])}</td>' if child["doc"] else '<td></td>'
            
            rows.append(f"""
            <tr>
                <td class="child-name"><a href="element_{html.escape(child['name'])}.html">{html.escape(child['name'])}</a></td>
                <td class="cardinality">{html.escape(cardinality)}</td>
                {doc_cell}
            </tr>""")
        
        return f"""
        <div class="section">
            <h2>Child Elements</h2>
            <table class="children-table">
                <thead>
                    <tr>
                        <th>Element</th>
                        <th>Cardinality</th>
                        <th>Description</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(rows)}
                </tbody>
            </table>
        </div>"""
    
    def generate_schema_page(self, schema_name):
        """Generate overview page for a schema."""
        tree = self.schemas.get(schema_name)
        if not tree:
            return
        
        # Get all top-level elements
        elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
        
        # Get target namespace
        root = tree.getroot()
        target_ns = root.get("targetNamespace", "")
        
        # Extract schema-level documentation
        schema_doc = self.extract_documentation(root)
        
        element_list = []
        for elem in elements:
            name = elem.get("name")
            doc = self.extract_documentation(elem)
            brief = doc.split(".")[0] if doc else ""
            element_list.append(f"""
            <tr>
                <td class="elem-name"><a href="element_{html.escape(name)}.html">{html.escape(name)}</a></td>
                <td class="elem-desc">{html.escape(brief)}</td>
            </tr>""")
        
        html_content = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{schema_name} - WiX v4 Schema</title>
    <style>
        {self.get_css()}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>{html.escape(schema_name)}.xsd</h1>
            <p class="namespace">Namespace: <code>{html.escape(target_ns)}</code></p>
        </div>
        
        <nav class="breadcrumb">
            <a href="index.html">Index</a> &raquo; {html.escape(schema_name)}
        </nav>
        
        {f'<div class="description">{self._format_doc(schema_doc)}</div>' if schema_doc else ''}
        
        <div class="section">
            <h2>Elements</h2>
            <table class="elements-table">
                <thead>
                    <tr>
                        <th>Element</th>
                        <th>Description</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(element_list)}
                </tbody>
            </table>
        </div>
        
        <div class="footer">
            Generated from WiX v4 schemas
        </div>
    </div>
</body>
</html>"""
        
        output_file = self.output_dir / f"schema_{schema_name}.html"
        output_file.write_text(html_content, encoding="utf-8")
    
    def generate_index(self):
        """Generate main index page."""
        schema_links = []
        for schema_name in sorted(self.schemas.keys()):
            tree = self.schemas[schema_name]
            elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
            
            # Get schema description
            root = tree.getroot()
            schema_doc = self.extract_documentation(root)
            brief = schema_doc.split(".")[0] if schema_doc else ""
            
            schema_links.append(f"""
            <tr>
                <td class="schema-name"><a href="schema_{html.escape(schema_name)}.html">{html.escape(schema_name)}.xsd</a></td>
                <td class="schema-elements">{len(elements)} elements</td>
                <td class="schema-desc">{html.escape(brief)}</td>
            </tr>""")
        
        # Generate element index
        element_links = []
        for elem_name in sorted(self.elements.keys()):
            schemas = [s for s, _ in self.elements[elem_name]]
            element_links.append(f"""
            <tr>
                <td class="elem-name"><a href="element_{html.escape(elem_name)}.html">{html.escape(elem_name)}</a></td>
                <td class="elem-schemas">{html.escape(", ".join(schemas))}</td>
            </tr>""")
        
        html_content = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>WiX v4 Schema Documentation</title>
    <style>
        {self.get_css()}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>WiX Toolset v4 Schema Documentation</h1>
            <p>Comprehensive reference for WiX v4 XML schemas</p>
        </div>
        
        <div class="section">
            <h2>Schema Files</h2>
            <table class="index-table">
                <thead>
                    <tr>
                        <th>Schema</th>
                        <th>Elements</th>
                        <th>Description</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(schema_links)}
                </tbody>
            </table>
        </div>
        
        <div class="section">
            <h2>All Elements</h2>
            <table class="index-table">
                <thead>
                    <tr>
                        <th>Element</th>
                        <th>Schema(s)</th>
                    </tr>
                </thead>
                <tbody>
                    {"".join(element_links)}
                </tbody>
            </table>
        </div>
        
        <div class="footer">
            Generated from WiX v4 schemas | <a href="https://wixtoolset.org">WiX Toolset</a>
        </div>
    </div>
</body>
</html>"""
        
        output_file = self.output_dir / "index.html"
        output_file.write_text(html_content, encoding="utf-8")
        print(f"\nIndex generated: {output_file}")
    
    def _build_relationships(self):
        """Build parent-child relationships between elements."""
        print("\nBuilding element relationships...")
        
        for element_name, occurrences in self.elements.items():
            for schema_name, element_node in occurrences:
                # Get child elements
                children = self.get_child_elements(element_node)
                for child in children:
                    child_name = child['name']
                    if child_name and not child_name.startswith('['):
                        self.relationships[element_name].append(child_name)
    
    def generate_mermaid_diagram(self, element_name=None):
        """Generate a Mermaid diagram for element relationships."""
        if element_name:
            # Generate diagram for specific element and its children
            return self._generate_element_mermaid(element_name)
        else:
            # Generate overall schema diagram
            return self._generate_schema_overview_mermaid()
    
    def _generate_element_mermaid(self, element_name):
        """Generate mermaid diagram for a specific element."""
        lines = ["```mermaid", "graph TD"]
        
        # Add the element
        lines.append(f"    {element_name}[{element_name}]")
        
        # Add children
        if element_name in self.relationships:
            for child in self.relationships[element_name]:
                child_id = child.replace('-', '_')
                lines.append(f"    {element_name} --> {child_id}[{child}]")
                
                # Add grandchildren (one level deep)
                if child in self.relationships:
                    for grandchild in self.relationships[child][:3]:  # Limit to 3 for readability
                        grandchild_id = grandchild.replace('-', '_')
                        lines.append(f"    {child_id} --> {grandchild_id}[{grandchild}]")
        
        lines.append("```")
        return "\n".join(lines)
    
    def _generate_schema_overview_mermaid(self):
        """Generate overview diagram of all schemas."""
        lines = ["```mermaid", "graph LR"]
        
        # Group elements by schema
        schema_elements = defaultdict(list)
        for element_name, occurrences in self.elements.items():
            for schema_name, _ in occurrences:
                schema_elements[schema_name].append(element_name)
        
        # Create schema nodes
        for schema_name in sorted(schema_elements.keys()):
            schema_id = schema_name.replace('-', '_')
            element_count = len(schema_elements[schema_name])
            lines.append(f"    {schema_id}[{schema_name}<br/>{element_count} elements]")
        
        lines.append("```")
        return "\n".join(lines)
    
    def generate_markdown_element_page(self, element_name, schema_name, element_node):
        """Generate Markdown page for a single element."""
        doc = self.extract_documentation(element_node)
        attrs = self.get_attributes(element_node)
        children = self.get_child_elements(element_node)
        
        # Get parent information
        parents = []
        for parent in element_node.xpath(".//xs:appinfo//xse:parent", namespaces={
            "xs": XS_NS,
            "xse": "http://wixtoolset.org/schemas/XmlSchemaExtension"
        }):
            parent_ref = parent.get("ref", "")
            parents.append(parent_ref)
        
        lines = [
            f"# {element_name}",
            "",
            f"**Schema:** `{schema_name}.xsd`",
            "",
        ]
        
        # Add description
        if doc:
            lines.extend([
                "## Description",
                "",
                doc.strip(),
                "",
            ])
        
        # Add mermaid diagram
        if element_name in self.relationships or parents:
            lines.extend([
                "## Element Hierarchy",
                "",
                self._generate_element_mermaid(element_name),
                "",
            ])
        
        # Add parents
        if parents:
            lines.extend([
                "## Parent Elements",
                "",
                "This element can be a child of:",
                "",
            ])
            for parent in parents:
                lines.append(f"- [{parent}](element_{parent}.md)")
            lines.append("")
        
        # Add attributes
        if attrs:
            lines.extend([
                "## Attributes",
                "",
                "| Name | Type | Use | Description |",
                "|------|------|-----|-------------|",
            ])
            for attr in attrs:
                name = attr['name'].replace('|', '\\|')
                type_str = attr['type'].replace('|', '\\|')
                use_str = f"**{attr['use']}**" if attr['use'] == 'required' else attr['use']
                desc = (attr['doc'] or '').replace('\n', ' ').replace('|', '\\|').strip()
                lines.append(f"| `{name}` | `{type_str}` | {use_str} | {desc} |")
            lines.append("")
        
        # Add child elements
        if children:
            lines.extend([
                "## Child Elements",
                "",
                "| Element | Cardinality | Description |",
                "|---------|-------------|-------------|",
            ])
            for child in children:
                name = child['name']
                if not name.startswith('['):
                    cardinality = f"{child['minOccurs']}..{child['maxOccurs']}"
                    desc = (child['doc'] or '').replace('\n', ' ').replace('|', '\\|').strip()
                    lines.append(f"| [{name}](element_{name}.md) | `{cardinality}` | {desc} |")
            lines.append("")
        
        # Add navigation
        lines.extend([
            "---",
            "",
            f"[← Back to {schema_name} Schema](schema_{schema_name}.md) | [↑ Back to Index](index.md)",
            "",
        ])
        
        markdown_content = "\n".join(lines)
        output_file = self.output_dir / f"element_{element_name}.md"
        output_file.write_text(markdown_content, encoding="utf-8")
    
    def generate_markdown_schema_page(self, schema_name):
        """Generate Markdown overview page for a schema."""
        tree = self.schemas.get(schema_name)
        if not tree:
            return
        
        # Get all top-level elements
        elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
        
        # Get target namespace
        root = tree.getroot()
        target_ns = root.get("targetNamespace", "")
        schema_doc = self.extract_documentation(root)
        
        lines = [
            f"# {schema_name}.xsd",
            "",
            f"**Namespace:** `{target_ns}`",
            "",
        ]
        
        if schema_doc:
            lines.extend([
                "## Description",
                "",
                schema_doc.strip(),
                "",
            ])
        
        # Add elements table
        lines.extend([
            "## Elements",
            "",
            "| Element | Description |",
            "|---------|-------------|",
        ])
        
        for elem in elements:
            name = elem.get("name")
            doc = self.extract_documentation(elem)
            brief = (doc.split(".")[0] if doc else "").replace('\n', ' ').replace('|', '\\|').strip()
            lines.append(f"| [{name}](element_{name}.md) | {brief} |")
        
        lines.extend([
            "",
            "---",
            "",
            "[↑ Back to Index](index.md)",
            "",
        ])
        
        markdown_content = "\n".join(lines)
        output_file = self.output_dir / f"schema_{schema_name}.md"
        output_file.write_text(markdown_content, encoding="utf-8")
    
    def generate_markdown_index(self):
        """Generate main Markdown index page."""
        lines = [
            "# WiX Toolset v4 Schema Documentation",
            "",
            "Comprehensive reference for WiX v4 XML schemas",
            "",
            "## Schema Overview",
            "",
            self._generate_schema_overview_mermaid(),
            "",
            "## Schema Files",
            "",
            "| Schema | Elements | Description |",
            "|--------|----------|-------------|",
        ]
        
        # Add schema links
        for schema_name in sorted(self.schemas.keys()):
            tree = self.schemas[schema_name]
            elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
            root = tree.getroot()
            schema_doc = self.extract_documentation(root)
            brief = (schema_doc.split(".")[0] if schema_doc else "").replace('\n', ' ').replace('|', '\\|').strip()
            
            lines.append(f"| [{schema_name}.xsd](schema_{schema_name}.md) | {len(elements)} | {brief} |")
        
        lines.extend([
            "",
            "## All Elements",
            "",
            "| Element | Schema(s) |",
            "|---------|-----------|",
        ])
        
        # Add element links
        for elem_name in sorted(self.elements.keys()):
            schemas = [s for s, _ in self.elements[elem_name]]
            schema_list = ", ".join(f"`{s}`" for s in schemas)
            lines.append(f"| [{elem_name}](element_{elem_name}.md) | {schema_list} |")
        
        lines.extend([
            "",
            "---",
            "",
            "*Generated from WiX v4 schemas* | [WiX Toolset](https://wixtoolset.org)",
            "",
        ])
        
        markdown_content = "\n".join(lines)
        output_file = self.output_dir / "index.md"
        output_file.write_text(markdown_content, encoding="utf-8")
        print(f"\nMarkdown index generated: {output_file}")
    
    def generate_asciidoc_element_page(self, element_name, schema_name, element_node):
        """Generate AsciiDoc page for a single element."""
        doc = self.extract_documentation(element_node)
        attrs = self.get_attributes(element_node)
        children = self.get_child_elements(element_node)
        
        # Get parent information
        parents = []
        for parent in element_node.xpath(".//xs:appinfo//xse:parent", namespaces={
            "xs": XS_NS,
            "xse": "http://wixtoolset.org/schemas/XmlSchemaExtension"
        }):
            parent_ref = parent.get("ref", "")
            parents.append(parent_ref)
        
        lines = [
            f"= {element_name}",
            ":schema: " + schema_name + ".xsd",
            ":toc:",
            ":toclevels: 2",
            "",
        ]
        
        # Add description
        if doc:
            lines.extend([
                "== Description",
                "",
                doc.strip(),
                "",
            ])
        
        # Add mermaid diagram
        if element_name in self.relationships or parents:
            lines.extend([
                "== Element Hierarchy",
                "",
                "[mermaid]",
                "....",
                self._generate_element_mermaid(element_name).replace("```mermaid\n", "").replace("```", ""),
                "....",
                "",
            ])
        
        # Add parents
        if parents:
            lines.extend([
                "== Parent Elements",
                "",
                "This element can be a child of:",
                "",
            ])
            for parent in parents:
                lines.append(f"* link:element_{parent}.adoc[{parent}]")
            lines.append("")
        
        # Add attributes
        if attrs:
            lines.extend([
                "== Attributes",
                "",
                "[cols=\"1,2,1,3\",options=\"header\"]",
                "|===",
                "|Name |Type |Use |Description",
                "",
            ])
            for attr in attrs:
                name = attr['name']
                type_str = attr['type']
                use_str = f"*{attr['use']}*" if attr['use'] == 'required' else attr['use']
                desc = (attr['doc'] or '').replace('\n', ' ').strip()
                lines.append(f"|`{name}` |`{type_str}` |{use_str} |{desc}")
            lines.extend([
                "|===",
                "",
            ])
        
        # Add child elements
        if children:
            lines.extend([
                "== Child Elements",
                "",
                "[cols=\"2,1,3\",options=\"header\"]",
                "|===",
                "|Element |Cardinality |Description",
                "",
            ])
            for child in children:
                name = child['name']
                if not name.startswith('['):
                    cardinality = f"{child['minOccurs']}..{child['maxOccurs']}"
                    desc = (child['doc'] or '').replace('\n', ' ').strip()
                    lines.append(f"|link:element_{name}.adoc[{name}] |`{cardinality}` |{desc}")
            lines.extend([
                "|===",
                "",
            ])
        
        # Add navigation
        lines.extend([
            "---",
            "",
            f"link:schema_{schema_name}.adoc[← Back to {schema_name} Schema] | link:index.adoc[↑ Back to Index]",
            "",
        ])
        
        asciidoc_content = "\n".join(lines)
        output_file = self.output_dir / f"element_{element_name}.adoc"
        output_file.write_text(asciidoc_content, encoding="utf-8")
    
    def generate_asciidoc_schema_page(self, schema_name):
        """Generate AsciiDoc overview page for a schema."""
        tree = self.schemas.get(schema_name)
        if not tree:
            return
        
        # Get all top-level elements
        elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
        
        # Get target namespace
        root = tree.getroot()
        target_ns = root.get("targetNamespace", "")
        schema_doc = self.extract_documentation(root)
        
        lines = [
            f"= {schema_name}.xsd",
            ":namespace: " + target_ns,
            ":toc:",
            ":toclevels: 2",
            "",
        ]
        
        if schema_doc:
            lines.extend([
                "== Description",
                "",
                schema_doc.strip(),
                "",
            ])
        
        # Add elements table
        lines.extend([
            "== Elements",
            "",
            "[cols=\"1,3\",options=\"header\"]",
            "|===",
            "|Element |Description",
            "",
        ])
        
        for elem in elements:
            name = elem.get("name")
            doc = self.extract_documentation(elem)
            brief = (doc.split(".")[0] if doc else "").replace('\n', ' ').strip()
            lines.append(f"|link:element_{name}.adoc[{name}] |{brief}")
        
        lines.extend([
            "|===",
            "",
            "---",
            "",
            "link:index.adoc[↑ Back to Index]",
            "",
        ])
        
        asciidoc_content = "\n".join(lines)
        output_file = self.output_dir / f"schema_{schema_name}.adoc"
        output_file.write_text(asciidoc_content, encoding="utf-8")
    
    def generate_asciidoc_index(self):
        """Generate main AsciiDoc index page."""
        lines = [
            "= WiX Toolset v4 Schema Documentation",
            ":toc:",
            ":toclevels: 2",
            "",
            "Comprehensive reference for WiX v4 XML schemas",
            "",
            "== Schema Overview",
            "",
            "[mermaid]",
            "....",
            self._generate_schema_overview_mermaid().replace("```mermaid\n", "").replace("```", ""),
            "....",
            "",
            "== Schema Files",
            "",
            "[cols=\"2,1,3\",options=\"header\"]",
            "|===",
            "|Schema |Elements |Description",
            "",
        ]
        
        # Add schema links
        for schema_name in sorted(self.schemas.keys()):
            tree = self.schemas[schema_name]
            elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
            root = tree.getroot()
            schema_doc = self.extract_documentation(root)
            brief = (schema_doc.split(".")[0] if schema_doc else "").replace('\n', ' ').strip()
            
            lines.append(f"|link:schema_{schema_name}.adoc[{schema_name}.xsd] |{len(elements)} |{brief}")
        
        lines.extend([
            "|===",
            "",
            "== All Elements",
            "",
            "[cols=\"1,2\",options=\"header\"]",
            "|===",
            "|Element |Schema(s)",
            "",
        ])
        
        # Add element links
        for elem_name in sorted(self.elements.keys()):
            schemas = [s for s, _ in self.elements[elem_name]]
            schema_list = ", ".join(f"`{s}`" for s in schemas)
            lines.append(f"|link:element_{elem_name}.adoc[{elem_name}] |{schema_list}")
        
        lines.extend([
            "|===",
            "",
            "---",
            "",
            "_Generated from WiX v4 schemas_ | https://wixtoolset.org[WiX Toolset]",
            "",
        ])
        
        asciidoc_content = "\n".join(lines)
        output_file = self.output_dir / "index.adoc"
        output_file.write_text(asciidoc_content, encoding="utf-8")
        print(f"\nAsciiDoc index generated: {output_file}")
    
    def generate_typst_element_page(self, element_name, schema_name, element_node):
        """Generate Typst page for a single element."""
        doc = self.extract_documentation(element_node)
        attrs = self.get_attributes(element_node)
        children = self.get_child_elements(element_node)
        
        # Get parent information
        parents = []
        for parent in element_node.xpath(".//xs:appinfo//xse:parent", namespaces={
            "xs": XS_NS,
            "xse": "http://wixtoolset.org/schemas/XmlSchemaExtension"
        }):
            parent_ref = parent.get("ref", "")
            parents.append(parent_ref)
        
        lines = [
            f"= {element_name}",
            "",
            "#set text(font: (\"Inter\", \"Roboto\", \"Helvetica Neue\", \"Arial\"), size: 11pt)",
            "#set page(numbering: \"1\", margin: (x: 2cm, y: 2cm))",
            "#show heading.where(level: 1): set text(size: 24pt, fill: rgb(\"#667eea\"))",
            "#show heading.where(level: 2): set text(size: 18pt, fill: rgb(\"#764ba2\"))",
            "",
            f"*Schema:* `{schema_name}.xsd`",
            "",
        ]
        
        # Add description
        if doc:
            lines.extend([
                "== Description",
                "",
                self.clean_html_for_typst(doc.strip()),
                "",
            ])
        
        # Add mermaid diagram reference (Typst doesn't natively support Mermaid, but we can note it)
        if element_name in self.relationships or parents:
            lines.extend([
                "== Element Hierarchy",
                "",
                "```",
                "// Hierarchy diagram (view in Markdown/AsciiDoc version for visual):",
            ])
            if element_name in self.relationships:
                lines.append(f"// {element_name} contains:")
                for child in self.relationships[element_name][:5]:
                    lines.append(f"//   - {child}")
            lines.extend([
                "```",
                "",
            ])
        
        # Add parents
        if parents:
            lines.extend([
                "== Parent Elements",
                "",
                "This element can be a child of:",
                "",
            ])
            for parent in parents:
                lines.append(f"- #link(\"element_{parent}.typ\")[{parent}]")
            lines.append("")
        
        # Add attributes
        if attrs:
            lines.extend([
                "== Attributes",
                "",
                "#table(",
                "  columns: (auto, auto, auto, 1fr),",
                "  align: (left, left, left, left),",
                "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
                "  inset: 8pt,",
                "  [*Name*], [*Type*], [*Use*], [*Description*],",
            ])
            for attr in attrs:
                name = attr['name']
                type_str = attr['type']
                use_str = f"*{attr['use']}*" if attr['use'] == 'required' else attr['use']
                desc = self.clean_html_for_typst((attr['doc'] or '')).replace('\n', ' ').strip()
                lines.append(f"  [`{name}`], [`{type_str}`], [{use_str}], [{desc}],")
            lines.extend([
                ")",
                "",
            ])
        
        # Add child elements
        if children:
            lines.extend([
                "== Child Elements",
                "",
                "#table(",
                "  columns: (auto, auto, 1fr),",
                "  align: (left, left, left),",
                "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
                "  inset: 8pt,",
                "  [*Element*], [*Cardinality*], [*Description*],",
            ])
            for child in children:
                name = child['name']
                if not name.startswith('['):
                    cardinality = f"{child['minOccurs']}..{child['maxOccurs']}"
                    desc = self.clean_html_for_typst((child['doc'] or '')).replace('\n', ' ').strip()
                    lines.append(f"  [#link(\"element_{name}.typ\")[{name}]], [`{cardinality}`], [{desc}],")
            lines.extend([
                ")",
                "",
            ])
        
        # Add navigation
        lines.extend([
            "#line(length: 100%, stroke: 0.5pt)",
            "",
            f"#link(\"schema_{schema_name}.typ\")[← Back to {schema_name} Schema] | #link(\"index.typ\")[↑ Back to Index]",
            "",
        ])
        
        typst_content = "\n".join(lines)
        output_file = self.output_dir / f"element_{element_name}.typ"
        output_file.write_text(typst_content, encoding="utf-8")
    
    def generate_typst_schema_page(self, schema_name):
        """Generate Typst overview page for a schema."""
        tree = self.schemas.get(schema_name)
        if not tree:
            return
        
        # Get all top-level elements
        elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
        
        # Get target namespace
        root = tree.getroot()
        target_ns = root.get("targetNamespace", "")
        schema_doc = self.extract_documentation(root)
        
        lines = [
            f"= {schema_name}.xsd",
            "",
            "#set text(font: (\"Inter\", \"Roboto\", \"Helvetica Neue\", \"Arial\"), size: 11pt)",
            "#set page(numbering: \"1\", margin: (x: 2cm, y: 2cm))",
            "#show heading.where(level: 1): set text(size: 24pt, fill: rgb(\"#667eea\"))",
            "#show heading.where(level: 2): set text(size: 18pt, fill: rgb(\"#764ba2\"))",
            "",
            f"*Namespace:* `{target_ns}`",
            "",
        ]
        
        if schema_doc:
            lines.extend([
                "== Description",
                "",
                schema_doc.strip(),
                "",
            ])
        
        # Add elements table
        lines.extend([
            "== Elements",
            "",
            "#table(",
            "  columns: (auto, 1fr),",
            "  align: (left, left),",
            "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
            "  inset: 8pt,",
            "  [*Element*], [*Description*],",
        ])
        
        for elem in elements:
            name = elem.get("name")
            doc = self.extract_documentation(elem)
            brief = (doc.split(".")[0] if doc else "").replace('\n', ' ').strip()
            lines.append(f"  [#link(\"element_{name}.typ\")[{name}]], [{brief}],")
        
        lines.extend([
            ")",
            "",
            "#line(length: 100%, stroke: 0.5pt)",
            "",
            "#link(\"index.typ\")[↑ Back to Index]",
            "",
        ])
        
        typst_content = "\n".join(lines)
        output_file = self.output_dir / f"schema_{schema_name}.typ"
        output_file.write_text(typst_content, encoding="utf-8")
    
    def generate_typst_index(self):
        """Generate main Typst index page."""
        lines = [
            "= WiX Toolset v4 Schema Documentation",
            "",
            "#set text(font: (\"Inter\", \"Roboto\", \"Helvetica Neue\", \"Arial\"), size: 11pt)",
            "#set page(numbering: \"1\", margin: (x: 2cm, y: 2cm))",
            "#show heading.where(level: 1): set text(size: 28pt, fill: rgb(\"#667eea\"))",
            "#show heading.where(level: 2): set text(size: 20pt, fill: rgb(\"#764ba2\"))",
            "#set document(title: \"WiX Toolset v4 Schema Documentation\")",
            "",
            "#align(center)[",
            "  #text(size: 14pt)[",
            "    Comprehensive reference for WiX v4 XML schemas",
            "  ]",
            "]",
            "",
            "#pagebreak()",
            "",
            "== Schema Files",
            "",
            "#table(",
            "  columns: (auto, auto, 1fr),",
            "  align: (left, left, left),",
            "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
            "  inset: 8pt,",
            "  [*Schema*], [*Elements*], [*Description*],",
        ]
        
        # Add schema links
        for schema_name in sorted(self.schemas.keys()):
            tree = self.schemas[schema_name]
            elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
            root = tree.getroot()
            schema_doc = self.extract_documentation(root)
            brief = (schema_doc.split(".")[0] if schema_doc else "").replace('\n', ' ').strip()
            
            lines.append(f"  [#link(\"schema_{schema_name}.typ\")[{schema_name}.xsd]], [{len(elements)}], [{brief}],")
        
        lines.extend([
            ")",
            "",
            "== All Elements",
            "",
            "#table(",
            "  columns: (auto, 1fr),",
            "  align: (left, left),",
            "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
            "  inset: 8pt,",
            "  [*Element*], [*Schema(s)*],",
        ])
        
        # Add element links
        for elem_name in sorted(self.elements.keys()):
            schemas = [s for s, _ in self.elements[elem_name]]
            schema_list = ", ".join(f"`{s}`" for s in schemas)
            lines.append(f"  [#link(\"element_{elem_name}.typ\")[{elem_name}]], [{schema_list}],")
        
        lines.extend([
            ")",
            "",
            "#line(length: 100%, stroke: 0.5pt)",
            "",
            "#align(center)[",
            "  _Generated from WiX v4 schemas_ | #link(\"https://wixtoolset.org\")[WiX Toolset]",
            "]",
            "",
        ])
        
        typst_content = "\n".join(lines)
        output_file = self.output_dir / "index.typ"
        output_file.write_text(typst_content, encoding="utf-8")
        print(f"\nTypst index generated: {output_file}")
    
    def generate_typst_unified(self):
        """Generate a unified Typst document with all content and working internal links."""
        lines = [
            "= WiX Toolset v4 Schema Documentation",
            "",
            "#set text(font: (\"Inter\", \"Roboto\", \"Helvetica Neue\", \"Arial\"), size: 11pt)",
            "#set page(numbering: \"1\", margin: (x: 2cm, y: 2cm))",
            "#show heading.where(level: 1): set text(size: 28pt, fill: rgb(\"#667eea\"))",
            "#show heading.where(level: 2): set text(size: 20pt, fill: rgb(\"#764ba2\"))",
            "#set document(title: \"WiX Toolset v4 Schema Documentation\")",
            "#outline()",
            "",
            "#pagebreak()",
            "",
        ]
        
        # Add schemas section
        lines.extend([
            "= Schemas",
            "",
        ])
        
        for schema_name in sorted(self.schemas.keys()):
            tree = self.schemas[schema_name]
            root = tree.getroot()
            target_ns = root.get("targetNamespace", "")
            schema_doc = self.extract_documentation(root)
            
            lines.extend([
                f"== {schema_name}.xsd <schema-{schema_name}>",
                "",
                f"*Namespace:* `{target_ns}`",
                "",
            ])
            
            if schema_doc:
                lines.extend([
                    self.clean_html_for_typst(schema_doc.strip()),
                    "",
                ])
            
            # List elements in this schema
            elements = tree.xpath("//xs:element[@name]", namespaces=NSMAP)
            if elements:
                lines.extend([
                    "*Elements in this schema:* ",
                ])
                elem_links = []
                for elem in elements:
                    name = elem.get("name")
                    elem_links.append(f"#link(<element-{name}>)[{name}]")
                lines.append(", ".join(elem_links))
                lines.append("")
            
            lines.append("#pagebreak()")
            lines.append("")
        
        # Add elements section
        lines.extend([
            "= Elements Reference",
            "",
        ])
        
        for element_name in sorted(self.elements.keys()):
            schema_name, element_node = self.elements[element_name][0]
            doc = self.extract_documentation(element_node)
            attrs = self.get_attributes(element_node)
            children = self.get_child_elements(element_node)
            
            # Get parent information
            parents = []
            for parent in element_node.xpath(".//xs:appinfo//xse:parent", namespaces={
                "xs": XS_NS,
                "xse": "http://wixtoolset.org/schemas/XmlSchemaExtension"
            }):
                parent_ref = parent.get("ref", "")
                parents.append(parent_ref)
            
            lines.extend([
                f"== {element_name} <element-{element_name}>",
                "",
                f"*Schema:* #link(<schema-{schema_name}>)[{schema_name}.xsd]",
                "",
            ])
            
            # Add description
            if doc:
                lines.extend([
                    "=== Description",
                    "",
                    self.clean_html_for_typst(doc.strip()),
                    "",
                ])
            
            # Add hierarchy info
            if element_name in self.relationships or parents:
                lines.extend([
                    "=== Element Hierarchy",
                    "",
                ])
                if parents:
                    parent_links = []
                    for parent in parents:
                        parent_links.append(f"#link(<element-{parent}>)[{parent}]")
                    lines.append(f"*Parent elements:* {', '.join(parent_links)}")
                    lines.append("")
                
                if element_name in self.relationships:
                    child_links = []
                    for child in self.relationships[element_name][:5]:
                        child_links.append(f"#link(<element-{child}>)[{child}]")
                    lines.append(f"*Child elements:* {', '.join(child_links)}")
                    lines.append("")
            
            # Add attributes
            if attrs:
                lines.extend([
                    "=== Attributes",
                    "",
                    "#table(",
                    "  columns: (auto, auto, auto, 1fr),",
                    "  align: (left, left, left, left),",
                    "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
                    "  inset: 8pt,",
                    "  [*Name*], [*Type*], [*Use*], [*Description*],",
                ])
                for attr in attrs:
                    name = attr['name']
                    type_str = attr['type']
                    use_str = f"*{attr['use']}*" if attr['use'] == 'required' else attr['use']
                    desc = self.clean_html_for_typst((attr['doc'] or '')).replace('\n', ' ').strip()
                    lines.append(f"  [`{name}`], [`{type_str}`], [{use_str}], [{desc}],")
                lines.extend([
                    ")",
                    "",
                ])
            
            # Add child elements
            if children:
                lines.extend([
                    "=== Child Elements",
                    "",
                    "#table(",
                    "  columns: (auto, auto, 1fr),",
                    "  align: (left, left, left),",
                    "  fill: (x, y) => if y == 0 { rgb(\"#667eea\") },",
                    "  inset: 8pt,",
                    "  [*Element*], [*Cardinality*], [*Description*],",
                ])
                for child in children:
                    name = child['name']
                    if not name.startswith('['):
                        cardinality = f"{child['minOccurs']}..{child['maxOccurs']}"
                        desc = self.clean_html_for_typst((child['doc'] or '')).replace('\n', ' ').strip()
                        lines.append(f"  [#link(<element-{name}>)[{name}]], [`{cardinality}`], [{desc}],")
                lines.extend([
                    ")",
                    "",
                ])
            
            lines.append("#pagebreak()")
            lines.append("")
        
        # Write unified document
        typst_content = "\n".join(lines)
        output_file = self.output_dir / "complete.typ"
        output_file.write_text(typst_content, encoding="utf-8")
        print(f"  Typst unified document: complete.typ (compile this for working links!)")
    
    def get_css(self):
        """Return CSS styles for the documentation."""
        return """
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            line-height: 1.6;
            color: #333;
            background: #f5f5f5;
        }
        
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            box-shadow: 0 0 20px rgba(0,0,0,0.1);
        }
        
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 2rem;
            border-bottom: 4px solid #5568d3;
        }
        
        .header h1 {
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
        }
        
        .header p {
            font-size: 1.1rem;
            opacity: 0.9;
        }
        
        .schema-name, .namespace {
            font-family: 'Courier New', monospace;
            font-size: 0.95rem;
        }
        
        .breadcrumb {
            background: #f8f9fa;
            padding: 0.75rem 2rem;
            border-bottom: 1px solid #dee2e6;
            font-size: 0.9rem;
        }
        
        .breadcrumb a {
            color: #667eea;
            text-decoration: none;
        }
        
        .breadcrumb a:hover {
            text-decoration: underline;
        }
        
        .section {
            padding: 2rem;
            border-bottom: 1px solid #e9ecef;
        }
        
        .section:last-child {
            border-bottom: none;
        }
        
        .section h2 {
            color: #667eea;
            margin-bottom: 1rem;
            font-size: 1.5rem;
            border-bottom: 2px solid #667eea;
            padding-bottom: 0.5rem;
        }
        
        .description {
            padding: 2rem;
            background: #f8f9fa;
            border-left: 4px solid #667eea;
            margin: 0;
        }
        
        .description p {
            margin-bottom: 0.75rem;
        }
        
        table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 1rem;
        }
        
        thead {
            background: #667eea;
            color: white;
        }
        
        th {
            padding: 0.75rem;
            text-align: left;
            font-weight: 600;
        }
        
        td {
            padding: 0.75rem;
            border-bottom: 1px solid #dee2e6;
        }
        
        tr:hover {
            background: #f8f9fa;
        }
        
        .attr-name, .child-name, .elem-name, .schema-name {
            font-family: 'Courier New', monospace;
            font-weight: 600;
            color: #764ba2;
        }
        
        .attr-type {
            font-family: 'Courier New', monospace;
            color: #666;
            font-size: 0.9rem;
        }
        
        .attr-use {
            font-size: 0.85rem;
            font-weight: 600;
            text-transform: uppercase;
        }
        
        .required {
            color: #dc3545;
        }
        
        .optional {
            color: #6c757d;
        }
        
        .cardinality {
            font-family: 'Courier New', monospace;
            color: #666;
            font-size: 0.9rem;
        }
        
        .doc {
            color: #555;
            font-size: 0.95rem;
        }
        
        .doc p {
            margin-bottom: 0.5rem;
        }
        
        a {
            color: #667eea;
            text-decoration: none;
        }
        
        a:hover {
            text-decoration: underline;
        }
        
        code {
            background: #f8f9fa;
            padding: 0.2rem 0.4rem;
            border-radius: 3px;
            font-family: 'Courier New', monospace;
            font-size: 0.9rem;
            color: #764ba2;
        }
        
        .footer {
            text-align: center;
            padding: 2rem;
            background: #f8f9fa;
            color: #6c757d;
            font-size: 0.9rem;
        }
        
        .index-table {
            font-size: 0.95rem;
        }
        
        .schema-elements, .elem-schemas {
            color: #6c757d;
            font-size: 0.9rem;
        }
        """
    
    def generate_all(self):
        """Generate all documentation."""
        # Download schemas
        schema_dir = self.download_schemas()
        
        # Parse schemas
        self.parse_schemas(schema_dir)
        
        # Determine which formats to generate
        formats = []
        if self.format in ["html", "both", "all"]:
            formats.append("html")
        if self.format in ["markdown", "both", "all"]:
            formats.append("markdown")
        if self.format in ["asciidoc", "all"]:
            formats.append("asciidoc")
        if self.format in ["typst", "all"]:
            formats.append("typst")
        
        # Generate pages
        print(f"\nGenerating documentation pages (format: {self.format})...")
        
        if "html" in formats:
            # Generate HTML documentation
            print("  Generating HTML documentation...")
            for schema_name in self.schemas:
                print(f"    HTML schema page: {schema_name}")
                self.generate_schema_page(schema_name)
            
            for element_name, occurrences in self.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"    HTML element page: {element_name}")
                self.generate_element_page(element_name, schema_name, element_node)
            
            self.generate_index()
        
        if "markdown" in formats:
            # Generate Markdown documentation
            print("  Generating Markdown documentation...")
            for schema_name in self.schemas:
                print(f"    Markdown schema page: {schema_name}")
                self.generate_markdown_schema_page(schema_name)
            
            for element_name, occurrences in self.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"    Markdown element page: {element_name}")
                self.generate_markdown_element_page(element_name, schema_name, element_node)
            
            self.generate_markdown_index()
        
        if "asciidoc" in formats:
            # Generate AsciiDoc documentation
            print("  Generating AsciiDoc documentation...")
            for schema_name in self.schemas:
                print(f"    AsciiDoc schema page: {schema_name}")
                self.generate_asciidoc_schema_page(schema_name)
            
            for element_name, occurrences in self.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"    AsciiDoc element page: {element_name}")
                self.generate_asciidoc_element_page(element_name, schema_name, element_node)
            
            self.generate_asciidoc_index()
        
        if "typst" in formats:
            # Generate Typst documentation
            print("  Generating Typst documentation...")
            for schema_name in self.schemas:
                print(f"    Typst schema page: {schema_name}")
                self.generate_typst_schema_page(schema_name)
            
            for element_name, occurrences in self.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"    Typst element page: {element_name}")
                self.generate_typst_element_page(element_name, schema_name, element_node)
            
            self.generate_typst_index()
            self.generate_typst_unified()
        
        print(f"\n✓ Documentation generated in: {self.output_dir.absolute()}")
        if "html" in formats:
            print(f"  HTML: Open {self.output_dir / 'index.html'} in your browser")
        if "markdown" in formats:
            print(f"  Markdown: See {self.output_dir / 'index.md'}")
        if "asciidoc" in formats:
            print(f"  AsciiDoc: See {self.output_dir / 'index.adoc'}")
        if "typst" in formats:
            print(f"  Typst: Compile {self.output_dir / 'complete.typ'} for full doc with working links")
            print(f"         Or compile individual files: {self.output_dir / 'index.typ'}, element_*.typ")


def main():
    parser = argparse.ArgumentParser(
        description="Generate documentation from WiX v4 XSD schemas"
    )
    parser.add_argument(
        "output_dir",
        nargs="?",
        default="wix_docs",
        help="Output directory for generated documentation (default: wix_docs)"
    )
    parser.add_argument(
        "-f", "--format",
        choices=["html", "markdown", "asciidoc", "typst", "both", "all"],
        default="html",
        help="Output format: html, markdown, asciidoc, typst, both (html+markdown), or all (default: html)"
    )
    parser.add_argument(
        "-s", "--schema-dir",
        help="Use local schema directory instead of downloading"
    )
    
    args = parser.parse_args()
    
    generator = SchemaDocGenerator(args.output_dir, format=args.format)
    
    if args.schema_dir:
        # Use local schemas
        print(f"Using local schemas from: {args.schema_dir}")
        generator.parse_schemas(Path(args.schema_dir))
        
        # Determine which formats to generate
        formats = []
        if args.format in ["html", "both", "all"]:
            formats.append("html")
        if args.format in ["markdown", "both", "all"]:
            formats.append("markdown")
        if args.format in ["asciidoc", "all"]:
            formats.append("asciidoc")
        if args.format in ["typst", "all"]:
            formats.append("typst")
        
        print(f"\nGenerating documentation pages (format: {args.format})...")
        
        if "html" in formats:
            for schema_name in generator.schemas:
                print(f"  HTML schema page: {schema_name}")
                generator.generate_schema_page(schema_name)
            
            for element_name, occurrences in generator.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"  HTML element page: {element_name}")
                generator.generate_element_page(element_name, schema_name, element_node)
            
            generator.generate_index()
        
        if "markdown" in formats:
            for schema_name in generator.schemas:
                print(f"  Markdown schema page: {schema_name}")
                generator.generate_markdown_schema_page(schema_name)
            
            for element_name, occurrences in generator.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"  Markdown element page: {element_name}")
                generator.generate_markdown_element_page(element_name, schema_name, element_node)
            
            generator.generate_markdown_index()
        
        if "asciidoc" in formats:
            for schema_name in generator.schemas:
                print(f"  AsciiDoc schema page: {schema_name}")
                generator.generate_asciidoc_schema_page(schema_name)
            
            for element_name, occurrences in generator.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"  AsciiDoc element page: {element_name}")
                generator.generate_asciidoc_element_page(element_name, schema_name, element_node)
            
            generator.generate_asciidoc_index()
        
        if "typst" in formats:
            for schema_name in generator.schemas:
                print(f"  Typst schema page: {schema_name}")
                generator.generate_typst_schema_page(schema_name)
            
            for element_name, occurrences in generator.elements.items():
                schema_name, element_node = occurrences[0]
                print(f"  Typst element page: {element_name}")
                generator.generate_typst_element_page(element_name, schema_name, element_node)
            
            generator.generate_typst_index()
            generator.generate_typst_unified()
        
        print(f"\n✓ Documentation generated in: {generator.output_dir.absolute()}")
        if "html" in formats:
            print(f"  HTML: Open {generator.output_dir / 'index.html'} in your browser")
        if "markdown" in formats:
            print(f"  Markdown: See {generator.output_dir / 'index.md'}")
        if "asciidoc" in formats:
            print(f"  AsciiDoc: See {generator.output_dir / 'index.adoc'}")
        if "typst" in formats:
            print(f"  Typst: Compile {generator.output_dir / 'complete.typ'} for full doc with working links")
            print(f"         Or compile individual files: {generator.output_dir / 'index.typ'}, element_*.typ")
    else:
        # Download and generate
        generator.generate_all()


if __name__ == "__main__":
    main()

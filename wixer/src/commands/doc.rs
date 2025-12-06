use crate::utils::find_wixer_root;
use anyhow::{Context, Result};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info};

pub fn run(output_dir: &Path, format: &str) -> Result<()> {
    let wixer_root = find_wixer_root().unwrap_or_else(|e| {
        error!("Error determining project root: {}", e);
        std::process::exit(1);
    });

    let schema_dir = wixer_root.join("cache").join("xml-schema");
    if !schema_dir.exists() {
        anyhow::bail!(
            "Schema directory not found at {}. Please run 'wixer init' first.",
            schema_dir.display()
        );
    }

    info!(
        output_dir = %output_dir.display(),
        format = %format,
        "Generating documentation"
    );

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // Create format-specific output directories
    let html_dir = output_dir.join("html");
    let md_dir = output_dir.join("markdown");
    let adoc_dir = output_dir.join("asciidoc");

    if format == "html" || format == "all" {
        fs::create_dir_all(&html_dir)
            .with_context(|| format!("Failed to create html directory: {}", html_dir.display()))?;
    }
    if format == "markdown" || format == "all" {
        fs::create_dir_all(&md_dir).with_context(|| {
            format!("Failed to create markdown directory: {}", md_dir.display())
        })?;
    }
    if format == "asciidoc" || format == "all" {
        fs::create_dir_all(&adoc_dir).with_context(|| {
            format!(
                "Failed to create asciidoc directory: {}",
                adoc_dir.display()
            )
        })?;
    }

    let mut schemas: HashMap<String, SchemaInfo> = HashMap::new();

    let mut schema_contents: HashMap<String, String> = HashMap::new();
    for entry in fs::read_dir(&schema_dir)
        .with_context(|| format!("Failed to read schema directory: {}", schema_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "xsd") {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read schema file: {}", path.display()))?;
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            schema_contents.insert(name, content);
        }
    }

    let mut element_index: HashMap<String, Vec<String>> = HashMap::new(); // element_name -> [schema_names]
    let mut element_relationships: HashMap<String, Vec<String>> = HashMap::new(); // parent -> [children]

    // First pass: Indexing, Schema Info, and Relationships
    for (schema_name, content) in &schema_contents {
        let doc = Document::parse(content).unwrap();

        let root = doc.root_element();
        let target_ns = root.attribute("targetNamespace").unwrap_or("").to_string();
        let schema_doc = extract_documentation(root);

        schemas.insert(
            schema_name.clone(),
            SchemaInfo {
                target_ns,
                documentation: schema_doc,
            },
        );

        for node in doc.descendants() {
            if node.tag_name().name() == "element" && node.has_attribute("name") {
                let name = node.attribute("name").unwrap().to_string();
                element_index
                    .entry(name.clone())
                    .or_default()
                    .push(schema_name.clone());

                // Build relationships
                let children = get_child_elements(node);
                let child_names: Vec<String> = children.into_iter().map(|c| c.name).collect();
                if !child_names.is_empty() {
                    element_relationships.insert(name, child_names);
                }
            }
        }
    }

    // Generate Index
    if format == "html" || format == "all" {
        generate_html_index(output_dir, &schemas, &element_index)?;
    }
    if format == "markdown" || format == "all" {
        generate_markdown_index(output_dir, &schemas, &element_index)?;
    }
    if format == "asciidoc" || format == "all" {
        generate_asciidoc_index(output_dir, &schemas, &element_index)?;
    }

    // Second pass: Generation
    for (schema_name, content) in &schema_contents {
        let doc = Document::parse(content).unwrap();

        // Generate Schema Page
        if format == "html" || format == "all" {
            generate_html_schema_page(&html_dir, schema_name, &doc)?;
        }
        if format == "markdown" || format == "all" {
            generate_markdown_schema_page(&md_dir, schema_name, &doc)?;
        }
        if format == "asciidoc" || format == "all" {
            generate_asciidoc_schema_page(&adoc_dir, schema_name, &doc)?;
        }

        // Generate Element Pages
        for node in doc.descendants() {
            if node.tag_name().name() == "element" && node.has_attribute("name") {
                if format == "html" || format == "all" {
                    generate_html_element_page(
                        &html_dir,
                        schema_name,
                        node,
                        &element_relationships,
                    )?;
                }
                if format == "markdown" || format == "all" {
                    generate_markdown_element_page(
                        &md_dir,
                        schema_name,
                        node,
                        &element_relationships,
                    )?;
                }
                if format == "asciidoc" || format == "all" {
                    generate_asciidoc_element_page(
                        &adoc_dir,
                        schema_name,
                        node,
                        &element_relationships,
                    )?;
                }
            }
        }
    }

    info!("Documentation generated in {}", output_dir.display());
    Ok(())
}

struct SchemaInfo {
    target_ns: String,
    documentation: Option<String>,
}

fn extract_documentation(node: Node) -> Option<String> {
    node.children()
        .find(|n| n.tag_name().name() == "annotation")
        .and_then(|n| {
            n.children()
                .find(|c| c.tag_name().name() == "documentation")
        })
        .and_then(|n| n.text().map(|t| t.trim().to_string()))
}

fn get_child_elements(node: Node) -> Vec<ChildInfo> {
    let mut children = Vec::new();

    // Simplified logic to find child element references
    // In XSD, children are typically defined in complexType -> sequence/choice/all -> element

    // Helper recursive function to find elements
    fn find_elements_recursive<'a>(n: Node<'a, 'a>, list: &mut Vec<ChildInfo>) {
        for child in n.children() {
            if child.tag_name().name() == "element" {
                let name = child
                    .attribute("ref")
                    .or_else(|| child.attribute("name"))
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    let min_occurs = child.attribute("minOccurs").unwrap_or("1").to_string();
                    let max_occurs = child.attribute("maxOccurs").unwrap_or("1").to_string();
                    let doc = extract_documentation(child);
                    list.push(ChildInfo {
                        name,
                        min_occurs,
                        max_occurs,
                        doc,
                    });
                }
            } else if matches!(
                child.tag_name().name(),
                "sequence" | "choice" | "all" | "complexType" | "complexContent" | "extension"
            ) {
                find_elements_recursive(child, list);
            }
        }
    }

    find_elements_recursive(node, &mut children);
    children
}

struct ChildInfo {
    name: String,
    min_occurs: String,
    max_occurs: String,
    doc: Option<String>,
}

fn generate_element_mermaid(
    element_name: &str,
    relationships: &HashMap<String, Vec<String>>,
) -> String {
    let mut lines = Vec::new();
    lines.push("graph TD".to_string());
    lines.push(format!("    {0}[{0}]", element_name));

    if let Some(children) = relationships.get(element_name) {
        for child in children {
            let child_id = child.replace('-', "_");
            lines.push(format!("    {} --> {}[{}]", element_name, child_id, child));

            // Grandchildren
            if let Some(grandchildren) = relationships.get(child) {
                for grandchild in grandchildren.iter().take(3) {
                    // Limit to 3
                    let grandchild_id = grandchild.replace('-', "_");
                    lines.push(format!(
                        "    {} --> {}[{}]",
                        child_id, grandchild_id, grandchild
                    ));
                }
            }
        }
    }
    lines.join("\n")
}

fn get_attributes(node: Node) -> Vec<AttributeInfo> {
    let mut attrs = Vec::new();

    // Helper to extract from a node that can contain attributes
    let extract_from = |n: Node, list: &mut Vec<AttributeInfo>| {
        for child in n.children() {
            if child.tag_name().name() == "attribute" {
                let name = child.attribute("name").unwrap_or("").to_string();
                if !name.is_empty() {
                    let type_name = child.attribute("type").unwrap_or("").to_string();
                    let use_attr = child.attribute("use").unwrap_or("optional").to_string();
                    let doc = extract_documentation(child);
                    list.push(AttributeInfo {
                        name,
                        type_name,
                        use_attr,
                        doc,
                    });
                }
            }
        }
    };

    // 1. Check if inline complexType
    if let Some(complex) = node
        .children()
        .find(|n| n.tag_name().name() == "complexType")
    {
        extract_from(complex, &mut attrs);

        // Check content models (simpleContent, complexContent)
        for content in complex.children() {
            if content.tag_name().name() == "simpleContent"
                || content.tag_name().name() == "complexContent"
            {
                if let Some(extension) = content
                    .children()
                    .find(|n| n.tag_name().name() == "extension")
                {
                    extract_from(extension, &mut attrs);
                }
            }
        }
    }

    // 2. Check if it refers to a type? (Not handled fully here, would need type lookup)

    attrs
}

struct AttributeInfo {
    name: String,
    type_name: String,
    use_attr: String,
    doc: Option<String>,
}

fn generate_markdown_index(
    output_dir: &Path,
    schemas: &HashMap<String, SchemaInfo>,
    elements: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let mut lines = Vec::new();
    lines.push("# WiX Toolset v4 Schema Documentation".to_string());
    lines.push("".to_string());
    lines.push("Comprehensive reference for WiX v4 XML schemas".to_string());
    lines.push("".to_string());

    lines.push("## Schema Files".to_string());
    lines.push("".to_string());
    lines.push("| Schema | Namespace | Description |".to_string());
    lines.push("|--------|-----------|-------------|".to_string());

    let mut sorted_schemas: Vec<_> = schemas.keys().collect();
    sorted_schemas.sort();

    for name in sorted_schemas {
        if let Some(info) = schemas.get(name) {
            let doc = info
                .documentation
                .as_deref()
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("");
            let doc_clean = doc.replace('\n', " ").trim().to_string();
            lines.push(format!(
                "| [{}.xsd](markdown/schema_{}.md) | `{}` | {} |",
                name, name, info.target_ns, doc_clean
            ));
        }
    }

    lines.push("".to_string());
    lines.push("## All Elements".to_string());
    lines.push("".to_string());
    lines.push("| Element | Schema(s) |".to_string());
    lines.push("|---------|-----------|".to_string());

    let mut sorted_elements: Vec<_> = elements.keys().collect();
    sorted_elements.sort();

    for name in sorted_elements {
        if let Some(schemas) = elements.get(name) {
            let schema_links = schemas
                .iter()
                .map(|s| format!("{}", s))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "| [{0}](markdown/element_{0}.md) | {1} |",
                name, schema_links
            ));
        }
    }

    lines.push("".to_string());
    lines.push("---".to_string());
    lines.push("*Generated by wixer*".to_string());

    let content = lines.join("\n");
    fs::write(output_dir.join("index.md"), content)?;
    Ok(())
}

fn generate_markdown_schema_page(
    output_dir: &Path,
    schema_name: &str,
    doc: &Document,
) -> Result<()> {
    let root = doc.root_element();
    let target_ns = root.attribute("targetNamespace").unwrap_or("");
    let schema_doc = extract_documentation(root);

    let mut lines = Vec::new();
    lines.push(format!("# {}.xsd", schema_name));
    lines.push("".to_string());
    lines.push(format!("**Namespace:** `{}`", target_ns));
    lines.push("".to_string());

    if let Some(doc) = schema_doc {
        lines.push("## Description".to_string());
        lines.push("".to_string());
        lines.push(doc);
        lines.push("".to_string());
    }

    lines.push("## Elements".to_string());
    lines.push("".to_string());
    lines.push("| Element | Description |".to_string());
    lines.push("|---------|-------------|".to_string());

    let mut elements: Vec<Node> = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "element" && node.has_attribute("name") {
            elements.push(node);
        }
    }

    for node in elements {
        let name = node.attribute("name").unwrap();
        let doc = extract_documentation(node);
        let brief = doc
            .as_deref()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .replace('\n', " ")
            .trim()
            .to_string();
        lines.push(format!("| [{0}](element_{0}.md) | {1} |", name, brief));
    }

    lines.push("".to_string());
    lines.push("---".to_string());
    lines.push("[↑ Back to Index](../index.md)".to_string());

    let content = lines.join("\n");
    fs::write(
        output_dir.join(format!("schema_{}.md", schema_name)),
        content,
    )?;
    Ok(())
}

fn generate_markdown_element_page(
    output_dir: &Path,
    schema_name: &str,
    node: Node,
    relationships: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let name = node.attribute("name").unwrap();
    let doc = extract_documentation(node);

    let mut lines = Vec::new();
    lines.push(format!("# {}", name));
    lines.push("".to_string());
    lines.push(format!("**Schema:** `{}.xsd`", schema_name));
    lines.push("".to_string());

    if let Some(d) = doc {
        lines.push("## Description".to_string());
        lines.push("".to_string());
        lines.push(d);
        lines.push("".to_string());
    }

    // Mermaid
    if relationships.contains_key(name) {
        lines.push("## Element Hierarchy".to_string());
        lines.push("".to_string());
        lines.push("```mermaid".to_string());
        lines.push(generate_element_mermaid(name, relationships));
        lines.push("```".to_string());
        lines.push("".to_string());
    }

    let attributes = get_attributes(node);
    if !attributes.is_empty() {
        lines.push("## Attributes".to_string());
        lines.push("".to_string());
        lines.push("| Name | Type | Use | Description |".to_string());
        lines.push("|------|------|-----|-------------|".to_string());

        for attr in attributes {
            let use_str = if attr.use_attr == "required" {
                format!("**{}**", attr.use_attr)
            } else {
                attr.use_attr
            };
            let doc_str = attr.doc.unwrap_or_default().replace('\n', " ");
            lines.push(format!(
                "| `{}` | `{}` | {} | {} |",
                attr.name, attr.type_name, use_str, doc_str
            ));
        }
        lines.push("".to_string());
    }

    let children = get_child_elements(node);
    if !children.is_empty() {
        lines.push("## Child Elements".to_string());
        lines.push("".to_string());
        lines.push("| Element | Cardinality | Description |".to_string());
        lines.push("|---------|-------------|-------------|".to_string());

        for child in children {
            let cardinality = format!("{}..{}", child.min_occurs, child.max_occurs);
            let doc_str = child.doc.unwrap_or_default().replace('\n', " ");
            lines.push(format!(
                "| [{0}](element_{0}.md) | `{1}` | {2} |",
                child.name, cardinality, doc_str
            ));
        }
        lines.push("".to_string());
    }

    lines.push("---".to_string());
    lines.push(format!(
        "[← Back to {} Schema](schema_{}.md) | [↑ Back to Index](../index.md)",
        schema_name, schema_name
    ));

    let content = lines.join("\n");
    fs::write(output_dir.join(format!("element_{}.md", name)), content)?;
    Ok(())
}

fn generate_html_index(
    output_dir: &Path,
    schemas: &HashMap<String, SchemaInfo>,
    elements: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let mut html = String::from(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"UTF-8\">\n<title>WiX v4 Schema Documentation</title>\n",
    );
    html.push_str(&get_css());
    html.push_str("</head>\n<body>\n<div class=\"container\">\n");
    html.push_str("<div class=\"header\"><h1>WiX Toolset v4 Schema Documentation</h1><p>Comprehensive reference for WiX v4 XML schemas</p></div>\n");

    html.push_str("<div class=\"section\"><h2>Schema Files</h2>\n<table class=\"index-table\"><thead><tr><th>Schema</th><th>Namespace</th><th>Description</th></tr></thead><tbody>\n");

    let mut sorted_schemas: Vec<_> = schemas.keys().collect();
    sorted_schemas.sort();

    for name in sorted_schemas {
        if let Some(info) = schemas.get(name) {
            let doc = info
                .documentation
                .as_deref()
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("");
            let doc_clean = doc.replace('\n', " ").trim().to_string();
            html.push_str(&format!(
                "<tr><td class=\"schema-name\"><a href=\"html/schema_{0}.html\">{0}.xsd</a></td><td><code>{1}</code></td><td class=\"schema-desc\">{2}</td></tr>\n",
                name, html_escape(&info.target_ns), html_escape(&doc_clean)
            ));
        }
    }
    html.push_str("</tbody></table></div>\n");

    html.push_str("<div class=\"section\"><h2>All Elements</h2>\n<table class=\"index-table\"><thead><tr><th>Element</th><th>Schema(s)</th></tr></thead><tbody>\n");
    let mut sorted_elements: Vec<_> = elements.keys().collect();
    sorted_elements.sort();

    for name in sorted_elements {
        if let Some(schemas) = elements.get(name) {
            let schema_links = schemas
                .iter()
                .map(|s| format!("{}", s))
                .collect::<Vec<_>>()
                .join(", ");
            html.push_str(&format!(
                "<tr><td class=\"elem-name\"><a href=\"html/element_{0}.html\">{0}</a></td><td class=\"elem-schemas\">{1}</td></tr>\n",
                name, html_escape(&schema_links)
            ));
        }
    }
    html.push_str("</tbody></table></div>\n");
    html.push_str("<div class=\"footer\">Generated by wixer</div>\n</div>\n</body>\n</html>");

    fs::write(output_dir.join("index.html"), html)?;
    Ok(())
}

fn generate_html_schema_page(output_dir: &Path, schema_name: &str, doc: &Document) -> Result<()> {
    let root = doc.root_element();
    let target_ns = root.attribute("targetNamespace").unwrap_or("");
    let schema_doc = extract_documentation(root);

    let mut html = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"UTF-8\">\n");
    html.push_str(&format!("<title>{} - WiX v4 Schema</title>\n", schema_name));
    html.push_str(&get_css());
    html.push_str("</head>\n<body>\n<div class=\"container\">\n");

    html.push_str(&format!(
        "<div class=\"header\"><h1>{}.xsd</h1>",
        schema_name
    ));
    html.push_str(&format!(
        "<p class=\"namespace\">Namespace: <code>{}</code></p></div>\n",
        html_escape(target_ns)
    ));

    html.push_str(&format!(
        "<nav class=\"breadcrumb\"><a href=\"../index.html\">Index</a> &raquo; {}</nav>\n",
        schema_name
    ));

    if let Some(doc) = schema_doc {
        html.push_str(&format!(
            "<div class=\"description\"><p>{}</p></div>\n",
            html_escape(&doc)
        ));
    }

    html.push_str("<div class=\"section\"><h2>Elements</h2>\n<table class=\"elements-table\"><thead><tr><th>Element</th><th>Description</th></tr></thead><tbody>\n");

    let mut elements: Vec<Node> = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "element" && node.has_attribute("name") {
            elements.push(node);
        }
    }

    for node in elements {
        let name = node.attribute("name").unwrap();
        let doc = extract_documentation(node);
        let brief = doc.as_deref().unwrap_or("").split('.').next().unwrap_or("");
        html.push_str(&format!(
            "<tr><td class=\"elem-name\"><a href=\"element_{0}.html\">{0}</a></td><td class=\"elem-desc\">{1}</td></tr>\n",
            name, html_escape(brief)
        ));
    }
    html.push_str("</tbody></table></div>\n");
    html.push_str("<div class=\"footer\">Generated by wixer</div>\n</div>\n</body>\n</html>");

    fs::write(
        output_dir.join(format!("schema_{}.html", schema_name)),
        html,
    )?;
    Ok(())
}

fn generate_html_element_page(
    output_dir: &Path,
    schema_name: &str,
    node: Node,
    relationships: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let name = node.attribute("name").unwrap();
    let doc = extract_documentation(node);

    let mut html = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"UTF-8\">\n");
    html.push_str(&format!("<title>{} - WiX v4 Schema</title>\n", name));
    html.push_str(&get_css());
    // Add Mermaid
    if relationships.contains_key(name) {
        html.push_str("<script type=\"module\">import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs'; mermaid.initialize({ startOnLoad: true });</script>\n");
    }
    html.push_str("</head>\n<body>\n<div class=\"container\">\n");

    html.push_str(&format!("<div class=\"header\"><h1>{}</h1>", name));
    html.push_str(&format!(
        "<p class=\"schema-name\">Schema: {}.xsd</p></div>\n",
        schema_name
    ));

    html.push_str(&format!("<nav class=\"breadcrumb\"><a href=\"../index.html\">Index</a> &raquo; <a href=\"schema_{0}.html\">{0}</a> &raquo; {1}</nav>\n", schema_name, name));

    if let Some(d) = doc {
        html.push_str(&format!(
            "<div class=\"description\"><p>{}</p></div>\n",
            html_escape(&d)
        ));
    }

    // Mermaid
    if relationships.contains_key(name) {
        html.push_str("<div class=\"section\"><h2>Element Hierarchy</h2>\n");
        html.push_str("<div class=\"mermaid\">\n");
        html.push_str(&generate_element_mermaid(name, relationships));
        html.push_str("\n</div>\n</div>\n");
    }

    let attributes = get_attributes(node);
    if !attributes.is_empty() {
        html.push_str("<div class=\"section\"><h2>Attributes</h2>\n<table class=\"attrs-table\"><thead><tr><th>Name</th><th>Type</th><th>Use</th><th>Description</th></tr></thead><tbody>\n");
        for attr in attributes {
            let use_class = if attr.use_attr == "required" {
                "required"
            } else {
                "optional"
            };
            html.push_str(&format!(
                "<tr><td class=\"attr-name\">{}</td><td class=\"attr-type\">{}</td><td class=\"attr-use {}\">{}</td><td class=\"doc\">{}</td></tr>\n",
                html_escape(&attr.name), html_escape(&attr.type_name), use_class, html_escape(&attr.use_attr), html_escape(attr.doc.as_deref().unwrap_or(""))
            ));
        }
        html.push_str("</tbody></table></div>\n");
    }

    let children = get_child_elements(node);
    if !children.is_empty() {
        html.push_str("<div class=\"section\"><h2>Child Elements</h2>\n<table class=\"children-table\"><thead><tr><th>Element</th><th>Cardinality</th><th>Description</th></tr></thead><tbody>\n");
        for child in children {
            let cardinality = format!("{}..{}", child.min_occurs, child.max_occurs);
            let doc_str = child.doc.unwrap_or_default();
            html.push_str(&format!(
                "<tr><td class=\"child-name\"><a href=\"element_{0}.html\">{0}</a></td><td class=\"cardinality\">{1}</td><td class=\"doc\">{2}</td></tr>\n",
                html_escape(&child.name), html_escape(&cardinality), html_escape(&doc_str)
            ));
        }
        html.push_str("</tbody></table></div>\n");
    }

    html.push_str("<div class=\"footer\">Generated by wixer</div>\n</div>\n</body>\n</html>");

    fs::write(output_dir.join(format!("element_{}.html", name)), html)?;
    Ok(())
}

fn generate_asciidoc_index(
    output_dir: &Path,
    schemas: &HashMap<String, SchemaInfo>,
    elements: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let mut adoc = String::from(
        "= WiX Toolset v4 Schema Documentation\n:toc:\n\nComprehensive reference for WiX v4 XML schemas\n\n",
    );

    adoc.push_str("== Schema Files\n\n[cols=\"1,1,2\",options=\"header\"]\n|===\n|Schema |Namespace |Description\n");

    let mut sorted_schemas: Vec<_> = schemas.keys().collect();
    sorted_schemas.sort();

    for name in sorted_schemas {
        if let Some(info) = schemas.get(name) {
            let doc = info
                .documentation
                .as_deref()
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("");
            let doc_clean = doc.replace('\n', " ").trim().to_string();
            adoc.push_str(&format!(
                "|link:asciidoc/schema_{0}.adoc[{0}.xsd] |`{1}` |{2}\n",
                name, info.target_ns, doc_clean
            ));
        }
    }
    adoc.push_str("|===\n\n");

    adoc.push_str(
        "== All Elements\n\n[cols=\"1,2\",options=\"header\"]\n|===\n|Element |Schema(s)\n",
    );
    let mut sorted_elements: Vec<_> = elements.keys().collect();
    sorted_elements.sort();

    for name in sorted_elements {
        if let Some(schemas) = elements.get(name) {
            let schema_links = schemas
                .iter()
                .map(|s| format!("link:asciidoc/schema_{}.adoc[{}]", s, s))
                .collect::<Vec<_>>()
                .join(", ");
            adoc.push_str(&format!(
                "|link:asciidoc/element_{0}.adoc[{0}] |{1}\n",
                name, schema_links
            ));
        }
    }
    adoc.push_str("|===\n");

    fs::write(output_dir.join("index.adoc"), adoc)?;
    Ok(())
}

fn generate_asciidoc_schema_page(
    output_dir: &Path,
    schema_name: &str,
    doc: &Document,
) -> Result<()> {
    let root = doc.root_element();
    let target_ns = root.attribute("targetNamespace").unwrap_or("");
    let schema_doc = extract_documentation(root);

    let mut adoc = format!(
        "= {}.xsd\n:toc:\n\n*Namespace:* `{}`\n\n",
        schema_name, target_ns
    );

    if let Some(doc) = schema_doc {
        adoc.push_str("== Description\n\n");
        adoc.push_str(&doc);
        adoc.push_str("\n\n");
    }

    adoc.push_str(
        "== Elements\n\n[cols=\"1,3\",options=\"header\"]\n|===\n|Element |Description\n",
    );

    let mut elements: Vec<Node> = Vec::new();
    for node in doc.descendants() {
        if node.tag_name().name() == "element" && node.has_attribute("name") {
            elements.push(node);
        }
    }

    for node in elements {
        let name = node.attribute("name").unwrap();
        let doc = extract_documentation(node);
        let brief = doc
            .as_deref()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .replace('\n', " ")
            .trim()
            .to_string();
        adoc.push_str(&format!("|link:element_{0}.adoc[{0}] |{1}\n", name, brief));
    }
    adoc.push_str("|===\n\nlink:../index.adoc[Back to Index]\n");

    fs::write(
        output_dir.join(format!("schema_{}.adoc", schema_name)),
        adoc,
    )?;
    Ok(())
}

fn generate_asciidoc_element_page(
    output_dir: &Path,
    schema_name: &str,
    node: Node,
    relationships: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let name = node.attribute("name").unwrap();
    let doc = extract_documentation(node);

    let mut adoc = format!("= {}\n:toc:\n\n*Schema:* `{}.xsd`\n\n", name, schema_name);

    if let Some(d) = doc {
        adoc.push_str("== Description\n\n");
        adoc.push_str(&d);
        adoc.push_str("\n\n");
    }

    // Mermaid
    if relationships.contains_key(name) {
        adoc.push_str("== Element Hierarchy\n\n");
        adoc.push_str("[mermaid]\n....\n");
        adoc.push_str(&generate_element_mermaid(name, relationships));
        adoc.push_str("\n....\n\n");
    }

    let attributes = get_attributes(node);
    if !attributes.is_empty() {
        adoc.push_str("== Attributes\n\n[cols=\"1,1,1,3\",options=\"header\"]\n|===\n|Name |Type |Use |Description\n");
        for attr in attributes {
            let use_str = if attr.use_attr == "required" {
                format!("*{}*", attr.use_attr)
            } else {
                attr.use_attr
            };
            let doc_str = attr.doc.unwrap_or_default().replace('\n', " ");
            adoc.push_str(&format!(
                "|`{}` |`{}` |{} |{}\n",
                attr.name, attr.type_name, use_str, doc_str
            ));
        }
        adoc.push_str("|===\n\n");
    }

    let children = get_child_elements(node);
    if !children.is_empty() {
        adoc.push_str("== Child Elements\n\n[cols=\"1,1,3\",options=\"header\"]\n|===\n|Element |Cardinality |Description\n");
        for child in children {
            let cardinality = format!("{}..{}", child.min_occurs, child.max_occurs);
            let doc_str = child.doc.unwrap_or_default().replace('\n', " ");
            adoc.push_str(&format!(
                "|link:element_{0}.adoc[{0}] |`{1}` |{2}\n",
                child.name, cardinality, doc_str
            ));
        }
        adoc.push_str("|===\n\n");
    }

    adoc.push_str(&format!(
        "link:schema_{0}.adoc[Back to Schema] | link:../index.adoc[Back to Index]\n",
        schema_name
    ));

    fs::write(output_dir.join(format!("element_{}.adoc", name)), adoc)?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn get_css() -> String {
    r#"<style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 1200px; margin: 0 auto; padding: 20px; }
        h1, h2, h3 { color: #2c3e50; }
        table { width: 100%; border-collapse: collapse; margin: 20px 0; }
        th, td { padding: 12px; text-align: left; border-bottom: 1px solid #ddd; }
        th { background-color: #f8f9fa; font-weight: 600; }
        tr:hover { background-color: #f5f5f5; }
        a { color: #3498db; text-decoration: none; }
        a:hover { text-decoration: underline; }
        code { background-color: #f8f9fa; padding: 2px 4px; border-radius: 4px; font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace; font-size: 0.9em; }
        .required { color: #e74c3c; font-weight: bold; }
        .breadcrumb { margin-bottom: 20px; color: #666; }
        .namespace { color: #666; font-family: monospace; }
        .footer { margin-top: 40px; padding-top: 20px; border-top: 1px solid #eee; color: #7f8c8d; font-size: 0.9em; text-align: center; }
    </style>"#.to_string()
}

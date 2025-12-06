mod backends;
mod generator;
mod mermaid;
mod parser;
mod types;

use crate::utils::find_wixer_root;
use anyhow::{Context, Result};
use backends::asciidoc::AsciiDocGenerator;
use backends::html::HtmlGenerator;
use backends::markdown::MarkdownGenerator;
use generator::{DocGenerator, IndexContext};
use parser::{extract_documentation, get_child_elements};
use roxmltree::Document;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{error, info};
use types::SchemaInfo;

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

    let mut generators: Vec<(Box<dyn DocGenerator>, std::path::PathBuf)> = Vec::new();

    if format == "html" || format == "all" {
        let html_dir = output_dir.join("html");
        fs::create_dir_all(&html_dir)
            .with_context(|| format!("Failed to create html directory: {}", html_dir.display()))?;
        generators.push((Box::new(HtmlGenerator), html_dir));
    }
    if format == "markdown" || format == "all" {
        let md_dir = output_dir.join("markdown");
        fs::create_dir_all(&md_dir).with_context(|| {
            format!("Failed to create markdown directory: {}", md_dir.display())
        })?;
        generators.push((Box::new(MarkdownGenerator), md_dir));
    }
    if format == "asciidoc" || format == "all" {
        let adoc_dir = output_dir.join("asciidoc");
        fs::create_dir_all(&adoc_dir).with_context(|| {
            format!(
                "Failed to create asciidoc directory: {}",
                adoc_dir.display()
            )
        })?;
        generators.push((Box::new(AsciiDocGenerator), adoc_dir));
    }

    let mut schemas: HashMap<String, SchemaInfo> = HashMap::new();
    let mut schema_contents: HashMap<String, String> = HashMap::new();

    for entry in fs::read_dir(&schema_dir)
        .with_context(|| format!("Failed to read schema directory: {}", schema_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "xsd") {
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

    let index_context = IndexContext {
        schemas: &schemas,
        elements: &element_index,
    };

    // Generate Index
    for (generator, dir) in &generators {
        generator.generate_index(dir, &index_context)?;
    }

    // Second pass: Generation
    for (schema_name, content) in &schema_contents {
        let doc = Document::parse(content).unwrap();

        // Generate Schema Page
        for (generator, dir) in &generators {
            generator.generate_schema_page(dir, schema_name, &doc)?;
        }

        // Generate Element Pages
        for node in doc.descendants() {
            if node.tag_name().name() == "element" && node.has_attribute("name") {
                for (generator, dir) in &generators {
                    generator.generate_element_page(
                        dir,
                        schema_name,
                        node,
                        &element_relationships,
                    )?;
                }
            }
        }
    }

    // Generate Landing Page
    if format == "all" || format == "html" {
        let mut html = String::from(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"UTF-8\">\n<title>WiX v4 Schema Documentation</title>\n",
        );
        html.push_str(
            "<style>body{font-family:sans-serif;max-width:800px;margin:40px auto;padding:0 20px;line-height:1.6}h1{color:#333}ul{list-style-type:none;padding:0}li{margin-bottom:10px}a{text-decoration:none;color:#0366d6;font-size:1.2em}a:hover{text-decoration:underline}</style>",
        );
        html.push_str("</head>\n<body>\n");
        html.push_str("<h1>WiX Toolset v4 Schema Documentation</h1>\n");
        html.push_str("<p>Select a documentation format:</p>\n<ul>\n");

        if format == "html" || format == "all" {
            html.push_str("<li><a href=\"html/index.html\">HTML Documentation</a></li>\n");
        }
        if format == "markdown" || format == "all" {
            html.push_str("<li><a href=\"markdown/index.md\">Markdown Documentation</a></li>\n");
        }
        if format == "asciidoc" || format == "all" {
            html.push_str("<li><a href=\"asciidoc/index.adoc\">AsciiDoc Documentation</a></li>\n");
        }

        html.push_str("</ul>\n</body>\n</html>");
        fs::write(output_dir.join("index.html"), html)?;
    }

    // Generate README.md
    let mut readme = String::from("# WiX Toolset v4 Schema Documentation\n\n");
    readme.push_str("This documentation was generated by wixer.\n\n");
    readme.push_str("## Available Formats\n\n");

    if format == "html" || format == "all" {
        readme.push_str("- [HTML Documentation](html/index.html)\n");
    }
    if format == "markdown" || format == "all" {
        readme.push_str("- [Markdown Documentation](markdown/index.md)\n");
    }
    if format == "asciidoc" || format == "all" {
        readme.push_str("- [AsciiDoc Documentation](asciidoc/index.adoc)\n");
    }

    fs::write(output_dir.join("README.md"), readme)?;

    info!("Documentation generated in {}", output_dir.display());
    Ok(())
}

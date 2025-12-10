//! Upgrade pipeline
//!
//! Orchestrates the transformation process:
//! 1. Extract preprocessor directives
//! 2. Parse XML
//! 3. Apply transformations
//! 4. Serialize
//! 5. Restore preprocessor directives
//!
//! For multi-file projects, use `upgrade_files` which:
//! 1. Analyzes all files to build cross-file context
//! 2. Upgrades each file with that shared context

use anyhow::{Context, Result};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use roxmltree::{Document, Node};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tracing::info;

use super::context::UpgradeContext;
use super::preprocessor::PreprocessorPreserver;
use super::transformations::{
    Transformation, V4_NAMESPACE, V4_UI_NAMESPACE, standard_transformations,
};
use crate::schema_mapping_v3v4::is_standard_directory;

/// Upgrade a WiX v3 file to v4 (single file, no cross-file context)
pub fn upgrade_file(input: &Path, output: &Path) -> Result<()> {
    let content =
        fs::read_to_string(input).with_context(|| format!("Failed to read {}", input.display()))?;

    // Build context from single file
    let mut ctx = UpgradeContext::new();
    ctx.analyze_content(&content)?;

    let upgraded = upgrade_content_with_context(&content, &ctx)?;

    fs::write(output, &upgraded)
        .with_context(|| format!("Failed to write {}", output.display()))?;

    info!("Upgraded {}", output.display());
    Ok(())
}

/// Upgrade multiple WiX v3 files to v4 with shared cross-file context
///
/// This function:
/// 1. Analyzes all input files to build a shared context
/// 2. Upgrades each file using that context
///
/// This enables cross-file transformations like:
/// - WIXUI_INSTALLDIR property in one file, UIRef in another
pub fn upgrade_files(files: &[(impl AsRef<Path>, impl AsRef<Path>)]) -> Result<()> {
    // Phase 1: Analyze all files to build context
    let mut ctx = UpgradeContext::new();
    for (input, _output) in files {
        let content = fs::read_to_string(input.as_ref())
            .with_context(|| format!("Failed to read {}", input.as_ref().display()))?;
        ctx.analyze_content(&content)?;
    }

    // Phase 2: Upgrade each file with shared context
    for (input, output) in files {
        let content = fs::read_to_string(input.as_ref())
            .with_context(|| format!("Failed to read {}", input.as_ref().display()))?;

        let upgraded = upgrade_content_with_context(&content, &ctx)?;

        fs::write(output.as_ref(), &upgraded)
            .with_context(|| format!("Failed to write {}", output.as_ref().display()))?;

        info!("Upgraded {}", output.as_ref().display());
    }

    Ok(())
}

/// Upgrade WiX v3 content to v4 (single file, builds its own context)
#[allow(dead_code)]
pub fn upgrade_content(content: &str) -> Result<String> {
    let mut ctx = UpgradeContext::new();
    ctx.analyze_content(content)?;
    upgrade_content_with_context(content, &ctx)
}

/// Upgrade WiX v3 content to v4 with provided context
pub fn upgrade_content_with_context(content: &str, ctx: &UpgradeContext) -> Result<String> {
    // Step 1: Extract preprocessor directives
    let mut preprocessor = PreprocessorPreserver::new();
    let clean_content = if PreprocessorPreserver::has_directives(content) {
        preprocessor.extract(content)
    } else {
        content.to_string()
    };

    // Step 1b: Extract prolog content (between <?xml?> and root element)
    // This preserves comments that would otherwise be lost during parsing
    let prolog_content = extract_prolog(&clean_content);

    // Step 2: Parse XML
    let doc = Document::parse(&clean_content).with_context(|| "Failed to parse XML")?;

    // Step 3: Transform
    let transformations = standard_transformations();
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // Write XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    // Process root element
    let root = doc.root_element();
    process_root(root, &mut writer, &transformations, ctx)?;

    // Step 4: Serialize
    let result = writer.into_inner().into_inner();
    let mut output = String::from_utf8(result).with_context(|| "Invalid UTF-8 in output")?;

    // Step 4b: Re-insert prolog content after XML declaration
    if !prolog_content.is_empty() {
        // Find end of XML declaration
        if let Some(decl_end) = output.find("?>") {
            let insert_pos = decl_end + 2;
            output.insert_str(insert_pos, &format!("\n{}", prolog_content));
        }
    }

    // Step 5: Restore preprocessor directives
    if preprocessor.directive_count() > 0 {
        output = preprocessor.restore(&output);
    }

    Ok(output)
}

/// Extract content between XML declaration and root element
fn extract_prolog(content: &str) -> String {
    // Find end of XML declaration (<?xml ... ?>)
    // We need to find the XML declaration specifically, not just any ?>
    let decl_end = if content.trim_start().starts_with("<?xml") {
        content.find("?>").map(|i| i + 2).unwrap_or(0)
    } else {
        0
    };

    // Find start of root element (<Wix or <Include)
    // Must be a tag start, not inside another tag
    let remaining = &content[decl_end..];
    let root_start = remaining
        .find("<Wix")
        .or_else(|| remaining.find("<Include"))
        .map(|i| decl_end + i)
        .unwrap_or(content.len());

    // Extract and trim the prolog content
    let prolog = content[decl_end..root_start].trim();

    // Don't return anything that looks like an XML element (but allow comments)
    if prolog.starts_with('<') && !prolog.starts_with("<!--") {
        return String::new();
    }

    prolog.to_string()
}

/// Process the root element (Wix or Include)
fn process_root(
    root: Node,
    writer: &mut Writer<Cursor<Vec<u8>>>,
    transformations: &[Box<dyn Transformation>],
    ctx: &UpgradeContext,
) -> Result<()> {
    match root.tag_name().name() {
        "Wix" => process_wix_element(root, writer, transformations, ctx),
        "Include" => process_include_element(root, writer, transformations, ctx),
        _ => process_node(root, writer, transformations, ctx),
    }
}

/// Process Wix root element
fn process_wix_element(
    node: Node,
    writer: &mut Writer<Cursor<Vec<u8>>>,
    transformations: &[Box<dyn Transformation>],
    ctx: &UpgradeContext,
) -> Result<()> {
    let mut elem = BytesStart::new("Wix");
    elem.push_attribute(("xmlns", V4_NAMESPACE));

    // Add UI namespace if needed (from cross-file context or local detection)
    let needs_ui = ctx.needs_ui_namespace || {
        // Fallback: check this file for UIRef
        fn check_node(n: &Node) -> bool {
            if n.tag_name().name() == "UIRef"
                && let Some(id) = n.attribute("Id")
                && id.starts_with("WixUI_")
            {
                return true;
            }
            n.children().any(|c| c.is_element() && check_node(&c))
        }
        check_node(&node)
    };

    if needs_ui {
        elem.push_attribute(("xmlns:ui", V4_UI_NAMESPACE));
    }

    // Copy other namespace declarations
    for attr in node.attributes() {
        if attr.name() != "xmlns" && attr.name().starts_with("xmlns:") {
            elem.push_attribute((attr.name(), attr.value()));
        }
    }

    writer.write_event(Event::Start(elem))?;

    // Process children
    for child in node.children() {
        if child.is_element() {
            // Check for Product -> Package transformation
            if child.tag_name().name() == "Product" {
                process_product_element(child, writer, transformations, ctx)?;
            } else {
                process_node(child, writer, transformations, ctx)?;
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new("Wix")))?;
    Ok(())
}

/// Process Include root element
fn process_include_element(
    node: Node,
    writer: &mut Writer<Cursor<Vec<u8>>>,
    transformations: &[Box<dyn Transformation>],
    ctx: &UpgradeContext,
) -> Result<()> {
    let mut elem = BytesStart::new("Include");
    elem.push_attribute(("xmlns", V4_NAMESPACE));

    for attr in node.attributes() {
        if attr.name() != "xmlns" {
            elem.push_attribute((attr.name(), attr.value()));
        }
    }

    let has_children = node.children().any(|c| c.is_element());
    if has_children {
        writer.write_event(Event::Start(elem))?;
        for child in node.children() {
            if child.is_element() {
                process_node(child, writer, transformations, ctx)?;
            }
        }
        writer.write_event(Event::End(BytesEnd::new("Include")))?;
    } else {
        writer.write_event(Event::Empty(elem))?;
    }

    Ok(())
}

/// Process Product element (converts to Package)
fn process_product_element(
    node: Node,
    writer: &mut Writer<Cursor<Vec<u8>>>,
    transformations: &[Box<dyn Transformation>],
    ctx: &UpgradeContext,
) -> Result<()> {
    let mut elem = BytesStart::new("Package");

    // Find inner Package element to merge its attributes
    let inner_package = node.children().find(|c| c.tag_name().name() == "Package");

    // Copy Product attributes, renaming Id to ProductCode
    for attr in node.attributes() {
        let (name, value) = match attr.name() {
            "Id" => ("ProductCode", attr.value()),
            other => (other, attr.value()),
        };
        elem.push_attribute((name, value));
    }

    // Merge inner Package attributes
    if let Some(inner) = inner_package {
        for attr in inner.attributes() {
            let name = match attr.name() {
                "InstallScope" => "Scope",
                "InstallerVersion" => continue, // Skip
                other => other,
            };
            // Don't duplicate
            let already_has = node.attributes().any(|a| {
                let mapped = if a.name() == "Id" {
                    "ProductCode"
                } else {
                    a.name()
                };
                mapped == name
            });
            if !already_has {
                elem.push_attribute((name, attr.value()));
            }
        }
    }

    writer.write_event(Event::Start(elem))?;

    // Process children, skipping inner Package
    for child in node.children() {
        if child.is_element() && child.tag_name().name() != "Package" {
            process_node(child, writer, transformations, ctx)?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new("Package")))?;
    Ok(())
}

/// Process a general node, applying transformations
fn process_node(
    node: Node,
    writer: &mut Writer<Cursor<Vec<u8>>>,
    transformations: &[Box<dyn Transformation>],
    ctx: &UpgradeContext,
) -> Result<()> {
    let tag_name = node.tag_name().name();

    // Try each transformation
    for transform in transformations {
        if transform.applies_to(&node, ctx) {
            let handled = transform.apply(&node, writer, ctx)?;
            if handled {
                return Ok(());
            }
            // If not fully handled, the transformation wrote the start tag
            // and we need to process children and close
            for child in node.children() {
                if child.is_element() {
                    process_node(child, writer, transformations, ctx)?;
                } else if child.is_text()
                    && let Some(text) = child.text()
                    && !text.trim().is_empty()
                {
                    writer.write_event(Event::Text(BytesText::new(text)))?;
                }
            }
            writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
            return Ok(());
        }
    }

    // Special handling for TARGETDIR - skip it but process children
    if tag_name == "Directory" && node.attribute("Id") == Some("TARGETDIR") {
        for child in node.children() {
            if child.is_element() {
                process_node(child, writer, transformations, ctx)?;
            }
        }
        return Ok(());
    }

    // Special handling for standard directories
    if tag_name == "Directory"
        && let Some(id) = node.attribute("Id")
        && is_standard_directory(id)
    {
        let mut elem = BytesStart::new("StandardDirectory");
        elem.push_attribute(("Id", id));

        let has_children = node.children().any(|c| c.is_element());
        if has_children {
            writer.write_event(Event::Start(elem))?;
            for child in node.children() {
                if child.is_element() {
                    process_node(child, writer, transformations, ctx)?;
                }
            }
            writer.write_event(Event::End(BytesEnd::new("StandardDirectory")))?;
        } else {
            writer.write_event(Event::Empty(elem))?;
        }
        return Ok(());
    }

    // Default: copy element as-is
    let mut elem = BytesStart::new(tag_name);
    for attr in node.attributes() {
        elem.push_attribute((attr.name(), attr.value()));
    }

    let has_element_children = node.children().any(|c| c.is_element());
    let has_text_children = node
        .children()
        .any(|c| c.is_text() && !c.text().unwrap_or("").trim().is_empty());

    if !has_element_children && !has_text_children {
        writer.write_event(Event::Empty(elem))?;
    } else {
        writer.write_event(Event::Start(elem))?;

        for child in node.children() {
            if child.is_element() {
                process_node(child, writer, transformations, ctx)?;
            } else if child.is_text()
                && let Some(text) = child.text()
                && !text.trim().is_empty()
            {
                writer.write_event(Event::Text(BytesText::new(text)))?;
            }
        }

        writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_upgrade() {
        let input = r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine"/>
  </Product>
</Wix>"#;

        let result = upgrade_content(input).unwrap();

        assert!(result.contains("http://wixtoolset.org/schemas/v4/wxs"));
        assert!(result.contains("<Package"));
        assert!(result.contains("ProductCode="));
        assert!(result.contains("Scope="));
        assert!(!result.contains("<Product"));
        assert!(!result.contains("InstallerVersion"));
    }

    #[test]
    fn test_preprocessor_preservation() {
        let input = r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <?include Variables.wxi?>
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package/>
  </Product>
</Wix>"#;

        let result = upgrade_content(input).unwrap();

        assert!(result.contains("<?include Variables.wxi?>"));
    }

    #[test]
    fn test_cdata_condition_not_double_escaped() {
        let input = r#"<?xml version="1.0"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Test" Version="1.0.0" Manufacturer="Test" UpgradeCode="{00000000-0000-0000-0000-000000000000}">
    <Package/>
    <Condition Message="Need PowerShell"><![CDATA[VERSION >= "3.0"]]></Condition>
  </Product>
</Wix>"#;

        let result = upgrade_content(input).unwrap();

        // Should have Launch element with properly escaped condition
        assert!(result.contains("<Launch"));
        // Should NOT have double-escaped entities like &gt; or &quot;
        assert!(
            !result.contains("&amp;gt;"),
            "Should not have double-escaped &gt;"
        );
        assert!(
            !result.contains("&amp;quot;"),
            "Should not have double-escaped &quot;"
        );
        // The >= should be escaped as &gt;= in the attribute (single escape)
        // and quotes should be &quot;
        println!("Result:\n{}", result);
    }
}

use anyhow::Result;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use roxmltree::{Document, Node};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tracing::info;

pub fn run(input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let content = fs::read_to_string(&input)?;
    let doc = Document::parse(&content)?;

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    let root = doc.root_element();
    if root.tag_name().name() == "Wix" {
        convert_wix_element(root, &mut writer)?;
    } else {
        // Pass through or warn?
        convert_node(root, &mut writer)?;
    }

    let result = writer.into_inner().into_inner();
    let output_path = output.unwrap_or(input);
    fs::write(&output_path, result)?;
    info!("Upgraded {}", output_path.display());
    Ok(())
}

fn convert_wix_element(node: Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    let mut elem = BytesStart::new("Wix");
    elem.push_attribute(("xmlns", "http://wixtoolset.org/schemas/v4/wxs"));

    // Copy other attributes (e.g. xmlns:xsi, xmlns:util)
    for attr in node.attributes() {
        if attr.name() != "xmlns" {
            elem.push_attribute((attr.name(), attr.value()));
        }
    }

    writer.write_event(Event::Start(elem))?;

    for child in node.children() {
        if child.is_element() {
            if child.tag_name().name() == "Product" {
                convert_product(child, writer)?;
            } else {
                convert_node(child, writer)?;
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new("Wix")))?;
    Ok(())
}

fn convert_product(node: Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    // Merge Product and inner Package
    let mut elem = BytesStart::new("Package");

    // Product attributes
    for attr in node.attributes() {
        if attr.name() == "Id" && attr.value() == "*" {
            continue; // Skip Id="*"
        }
        elem.push_attribute((attr.name(), attr.value()));
    }

    // Find inner Package
    let inner_package = node.children().find(|n| n.tag_name().name() == "Package");
    if let Some(pkg) = inner_package {
        for attr in pkg.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }
    }

    writer.write_event(Event::Start(elem))?;

    for child in node.children() {
        if child.is_element() {
            if child.tag_name().name() == "Package" {
                continue; // Skip, already merged
            }
            convert_node(child, writer)?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new("Package")))?;
    Ok(())
}

fn convert_node(node: Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    // Simplistic name handling: Use existing name (usually local name in roxmltree if not namespaced correctly?)
    // roxmltree::Node::tag_name().name() returns local name.
    // We assume default namespace is inherited.
    let mut elem = BytesStart::new(node.tag_name().name());
    for attr in node.attributes() {
        elem.push_attribute((attr.name(), attr.value()));
    }

    // If empty and no text, write empty
    let has_element_children = node.children().any(|c| c.is_element());
    let has_text_children = node
        .children()
        .any(|c| c.is_text() && !c.text().unwrap_or("").trim().is_empty());

    if !has_element_children && !has_text_children {
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
    }

    writer.write_event(Event::Start(elem))?;

    for child in node.children() {
        if child.is_element() {
            convert_node(child, writer)?;
        } else if child.is_text() {
            if let Some(text) = child.text() {
                if !text.trim().is_empty() {
                    writer.write_event(Event::Text(BytesText::new(text)))?;
                }
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new(node.tag_name().name())))?;
    Ok(())
}

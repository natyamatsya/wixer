use crate::schema_mapping_v3v4::is_standard_directory;
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
    // Merge Product and inner Package into WiX v4 Package element
    let mut elem = BytesStart::new("Package");

    // Product attributes - handle v3→v4 attribute mapping
    for attr in node.attributes() {
        match attr.name() {
            "Id" => {
                // In v3, Product/@Id was a GUID (or "*" for auto-generate)
                // In v4, this becomes Package/@ProductCode (if not "*")
                if attr.value() != "*" {
                    elem.push_attribute(("ProductCode", attr.value()));
                }
                // Skip - don't add as "Id" since v4 Package/@Id must be an identifier
            }
            "Language" => {
                // Language stays the same but consider adding as InstallerVersion default
                elem.push_attribute((attr.name(), attr.value()));
            }
            _ => {
                elem.push_attribute((attr.name(), attr.value()));
            }
        }
    }

    // Find inner Package and merge its attributes
    let inner_package = node.children().find(|n| n.tag_name().name() == "Package");
    if let Some(pkg) = inner_package {
        for attr in pkg.attributes() {
            // Map v3 Package attributes to v4
            match attr.name() {
                "InstallScope" => {
                    // InstallScope -> Scope in v4
                    elem.push_attribute(("Scope", attr.value()));
                }
                "Compressed" => {
                    elem.push_attribute((attr.name(), attr.value()));
                }
                _ => {
                    elem.push_attribute((attr.name(), attr.value()));
                }
            }
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
    let tag_name = node.tag_name().name();
    
    // Handle Directory -> StandardDirectory conversion for well-known directories
    if tag_name == "Directory" {
        if let Some(id) = node.attribute("Id") {
            // TARGETDIR is implicit in v4 - skip it entirely and process children
            if id == "TARGETDIR" {
                for child in node.children() {
                    if child.is_element() {
                        convert_node(child, writer)?;
                    }
                }
                return Ok(());
            }
            
            // Other standard directories become StandardDirectory
            if is_standard_directory(id) {
                let name = node.attribute("Name");
                let is_standard_ref = name.is_none() || name == Some("SourceDir") || name == Some(id);
                
                if is_standard_ref {
                    let mut elem = BytesStart::new("StandardDirectory");
                    elem.push_attribute(("Id", id));
                    
                    let has_children = node.children().any(|c| c.is_element());
                    
                    if has_children {
                        writer.write_event(Event::Start(elem))?;
                        for child in node.children() {
                            if child.is_element() {
                                convert_node(child, writer)?;
                            }
                        }
                        writer.write_event(Event::End(BytesEnd::new("StandardDirectory")))?;
                    } else {
                        writer.write_event(Event::Empty(elem))?;
                    }
                    return Ok(());
                }
            }
        }
    }
    
    let mut elem = BytesStart::new(tag_name);
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
        } else if child.is_text()
            && let Some(text) = child.text()
            && !text.trim().is_empty()
        {
            writer.write_event(Event::Text(BytesText::new(text)))?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new(node.tag_name().name())))?;
    Ok(())
}

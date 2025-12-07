use crate::schema_mapping_v3v4::is_standard_directory;
use anyhow::Result;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use roxmltree::{Document, Node};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use regex::Regex;
use tracing::info;

/// Text-based upgrade for files with preprocessor directives that can't be parsed as XML
fn upgrade_text_based(content: &str) -> String {
    let mut result = content.to_string();
    
    // Namespace upgrade
    result = result.replace(
        "http://schemas.microsoft.com/wix/2006/wi",
        "http://wixtoolset.org/schemas/v4/wxs"
    );
    
    // Add namespace to Include elements that don't have one
    if result.contains("<Include>") && !result.contains("xmlns=") {
        result = result.replace("<Include>", r#"<Include xmlns="http://wixtoolset.org/schemas/v4/wxs">"#);
    }
    
    // Product -> Package transformation
    // In v4, Product element becomes Package, and inner Package attributes merge in
    // Step 1: Extract Product attributes and convert to Package
    if let Ok(re) = Regex::new(r#"<Product\s+([^>]*)>"#) {
        result = re.replace(&result, |caps: &regex::Captures| {
            let attrs = &caps[1];
            // Rename Id to ProductCode
            let attrs = attrs.replace("Id=", "ProductCode=");
            format!("<Package {}>", attrs)
        }).to_string();
    }
    
    // Step 2: Remove the inner <Package .../> element (its attributes should be on outer Package)
    // First, try to extract Scope from inner Package
    let mut scope_attr = String::new();
    if let Ok(re) = Regex::new(r#"<Package[^>]*InstallScope="([^"]*)"[^>]*/>"#) {
        if let Some(caps) = re.captures(&result) {
            scope_attr = format!(" Scope=\"{}\"", &caps[1]);
        }
    }
    if let Ok(re) = Regex::new(r#"<Package[^>]*Scope="([^"]*)"[^>]*/>"#) {
        if let Some(caps) = re.captures(&result) {
            scope_attr = format!(" Scope=\"{}\"", &caps[1]);
        }
    }
    
    // Remove inner Package element
    if let Ok(re) = Regex::new(r#"\s*<Package[^>]*/>\s*"#) {
        result = re.replace(&result, "\n").to_string();
    }
    
    // Add Scope to outer Package if we found it
    if !scope_attr.is_empty() && !result.contains("Scope=") {
        result = result.replace("<Package ", &format!("<Package{} ", scope_attr));
    }
    
    // Step 3: Convert </Product> to </Package>
    result = result.replace("</Product>", "</Package>");
    
    // InstallScope -> Scope (for any remaining)
    result = result.replace("InstallScope=", "Scope=");
    
    // BinaryKey -> BinaryRef
    result = result.replace("BinaryKey=", "BinaryRef=");
    
    // FileKey -> FileRef  
    result = result.replace("FileKey=", "FileRef=");
    
    // WixCA -> Wix4UtilCA_X86 for WixQuietExec
    result = result.replace(r#"BinaryRef="WixCA""#, r#"BinaryRef="Wix4UtilCA_X86""#);
    
    // Remove InstallerVersion (has better defaults in v4)
    if let Ok(re) = Regex::new(r#"\s*InstallerVersion="[^"]*""#) {
        result = re.replace_all(&result, "").to_string();
    }
    
    // Convert Condition element to Launch element
    // <Condition Message="...">condition text</Condition> -> <Launch Condition="condition text" Message="..."/>
    if let Ok(re) = Regex::new(r#"<Condition\s+Message="([^"]*)">\s*(<!\[CDATA\[)?(.*?)(]]>)?\s*</Condition>"#) {
        result = re.replace_all(&result, |caps: &regex::Captures| {
            let message = &caps[1];
            let condition = caps.get(3).map_or("", |m| m.as_str()).trim();
            // Escape quotes in condition for XML attribute
            let condition = condition.replace('"', "&quot;");
            format!(r#"<Launch Condition="{}" Message="{}"/>"#, condition, message)
        }).to_string();
    }
    
    // Convert Custom element inner text to Condition attribute
    // <Custom Action='...' ...>condition</Custom> -> <Custom Action='...' ... Condition="condition"/>
    if let Ok(re) = Regex::new(r#"<Custom\s+([^>]*)>([^<]+)</Custom>"#) {
        result = re.replace_all(&result, |caps: &regex::Captures| {
            let attrs = &caps[1];
            let condition = caps[2].trim();
            if condition.is_empty() {
                format!("<Custom {}/>", attrs)
            } else {
                format!(r#"<Custom {} Condition="{}"/>"#, attrs, condition)
            }
        }).to_string();
    }
    
    // Handle Directory structure transformation for v4
    // Remove TARGETDIR wrapper and convert standard directories
    // Pattern: <Directory Id="TARGETDIR"...>\n      <Directory Id="ProgramFilesFolder">
    // becomes: <StandardDirectory Id="ProgramFilesFolder">
    if let Ok(re) = Regex::new(r#"<Directory\s+Id="TARGETDIR"[^>]*>\s*\n\s*<Directory\s+Id="(ProgramFilesFolder|ProgramFiles64Folder)">"#) {
        result = re.replace_all(&result, |caps: &regex::Captures| {
            format!(r#"<StandardDirectory Id="{}">"#, &caps[1])
        }).to_string();
    }
    
    // Fix closing tags: </Directory>\n    </Directory> -> </StandardDirectory>
    // This handles the case where we removed TARGETDIR and ProgramFilesFolder
    if let Ok(re) = Regex::new(r#"</Directory>\s*\n\s*</Directory>(\s*\n\s*<ComponentGroup)"#) {
        result = re.replace_all(&result, "</StandardDirectory>$1").to_string();
    }
    
    result
}

pub fn run(input: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let content = fs::read_to_string(&input)?;
    let output_path = output.unwrap_or_else(|| input.clone());
    
    // Check if this is a file with preprocessor directives
    // These can't be parsed as XML, so do text-based transformations
    if content.contains("<?if") || content.contains("<?define") || content.contains("<?include") {
        let upgraded = upgrade_text_based(&content);
        fs::write(&output_path, &upgraded)?;
        info!("Upgraded (text): {}", output_path.display());
        return Ok(());
    }
    
    let doc = Document::parse(&content)?;

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    let root = doc.root_element();
    match root.tag_name().name() {
        "Wix" => convert_wix_element(root, &mut writer)?,
        "Include" => convert_include_element(root, &mut writer)?,
        _ => convert_node(root, &mut writer)?,
    }

    let result = writer.into_inner().into_inner();
    fs::write(&output_path, result)?;
    info!("Upgraded {}", output_path.display());
    Ok(())
}

fn convert_include_element(node: Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<()> {
    // Include files (.wxi) - add v4 namespace
    let mut elem = BytesStart::new("Include");
    elem.push_attribute(("xmlns", "http://wixtoolset.org/schemas/v4/wxs"));
    
    for attr in node.attributes() {
        if attr.name() != "xmlns" {
            elem.push_attribute((attr.name(), attr.value()));
        }
    }
    
    let has_children = node.children().any(|c| c.is_element() || (c.is_text() && !c.text().unwrap_or("").trim().is_empty()));
    
    if has_children {
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
        writer.write_event(Event::End(BytesEnd::new("Include")))?;
    } else {
        writer.write_event(Event::Empty(elem))?;
    }
    
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
    
    // Handle Condition under Package -> Launch element
    if tag_name == "Condition" {
        // Get the condition text and Message attribute
        let message = node.attribute("Message").unwrap_or("Installation cannot continue.");
        let condition_text = node.children()
            .filter_map(|c| c.text())
            .collect::<String>();
        let condition_text = condition_text.trim();
        
        if !condition_text.is_empty() {
            let mut elem = BytesStart::new("Launch");
            // In v4, Launch/@Condition is the condition that must be TRUE to continue
            // (same as v3 Condition element)
            elem.push_attribute(("Condition", condition_text));
            elem.push_attribute(("Message", message));
            writer.write_event(Event::Empty(elem))?;
            return Ok(());
        }
    }
    
    // Handle Custom element - inner text becomes Condition attribute
    if tag_name == "Custom" {
        let mut elem = BytesStart::new("Custom");
        
        // Copy existing attributes
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }
        
        // Convert inner text to Condition attribute
        let condition_text = node.children()
            .filter_map(|c| c.text())
            .collect::<String>();
        let condition_text = condition_text.trim();
        
        if !condition_text.is_empty() {
            elem.push_attribute(("Condition", condition_text));
        }
        
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
    }
    
    // Handle SetProperty element - inner text becomes Condition attribute
    if tag_name == "SetProperty" {
        let mut elem = BytesStart::new("SetProperty");
        
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }
        
        let condition_text = node.children()
            .filter_map(|c| c.text())
            .collect::<String>();
        let condition_text = condition_text.trim();
        
        if !condition_text.is_empty() {
            elem.push_attribute(("Condition", condition_text));
        }
        
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
    }
    
    // Handle Publish element - inner text becomes Condition attribute (same as Custom)
    if tag_name == "Publish" {
        let mut elem = BytesStart::new("Publish");
        
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }
        
        let condition_text = node.children()
            .filter_map(|c| c.text())
            .collect::<String>();
        let condition_text = condition_text.trim();
        
        if !condition_text.is_empty() {
            elem.push_attribute(("Condition", condition_text));
        }
        
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
    }
    
    // Handle CustomAction - check for WixQuietExec pattern
    if tag_name == "CustomAction" {
        let binary_key = node.attribute("BinaryKey").or_else(|| node.attribute("BinaryRef"));
        let dll_entry = node.attribute("DllEntry");
        
        // Check if this is a WixQuietExec custom action that needs conversion
        if binary_key == Some("WixCA") && (dll_entry == Some("WixQuietExec") || dll_entry == Some("WixQuietExec64")) {
            // In WiX v4+, WixQuietExec is handled differently
            // The SetProperty that sets the command line should use WixQuietExecCmdLine property
            // And the CustomAction should reference the util extension
            // For now, we'll convert to the new syntax
            let mut elem = BytesStart::new("CustomAction");
            
            for attr in node.attributes() {
                match attr.name() {
                    "BinaryKey" | "BinaryRef" => {
                        // Change WixCA to Wix4UtilCA_X86 or Wix4UtilCA_X64
                        elem.push_attribute(("BinaryRef", "Wix4UtilCA_X86"));
                    }
                    "DllEntry" => {
                        // Keep the DllEntry but it's now in the util extension
                        elem.push_attribute((attr.name(), attr.value()));
                    }
                    other => {
                        elem.push_attribute((other, attr.value()));
                    }
                }
            }
            
            writer.write_event(Event::Empty(elem))?;
            return Ok(());
        }
        
        // Regular CustomAction - just rename attributes
        let mut elem = BytesStart::new("CustomAction");
        
        for attr in node.attributes() {
            let new_name = match attr.name() {
                "BinaryKey" => "BinaryRef",
                "FileKey" => "FileRef",
                other => other,
            };
            elem.push_attribute((new_name, attr.value()));
        }
        
        // CustomAction is typically empty
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
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

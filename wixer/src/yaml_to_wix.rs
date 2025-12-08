use crate::common::AttributeStrategy;
use crate::constants::{
    ATTRIBUTES_PROPERTY, COMMENT_PROPERTY, CPACK_VAR_KEY, CPACK_VAR_TOKEN, CPACK_VAR_XML,
    ELEMENTS_PROPERTY, TEXT_PROPERTIES,
};
use anyhow::{Context, Result};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use regex::Regex;
use serde_yaml::Value;
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tracing::info;


/// Resolve `!include` tags in YAML content
/// Returns the processed YAML with includes resolved
fn resolve_includes(content: &str, base_path: &Path, visited: &mut HashSet<std::path::PathBuf>) -> Result<String> {
    // Pattern to match !include tags: `!include filename` or `!include "filename"`
    let include_re = Regex::new(r#"!include\s+["']?([^"'\s\n]+)["']?"#).unwrap();
    
    let mut result = content.to_string();
    
    // Find all includes
    let matches: Vec<_> = include_re.captures_iter(content).collect();
    
    for cap in matches.iter().rev() {
        // Process in reverse to maintain correct positions
        let full_match = cap.get(0).unwrap();
        let include_path_str = cap.get(1).unwrap().as_str();
        
        // Resolve path relative to the base file
        let include_path = base_path.parent().unwrap_or(Path::new(".")).join(include_path_str);
        let canonical = include_path.canonicalize()
            .with_context(|| format!("Failed to resolve include path: {}", include_path.display()))?;
        
        // Check for cycles
        if visited.contains(&canonical) {
            anyhow::bail!("Circular include detected: {}", canonical.display());
        }
        visited.insert(canonical.clone());
        
        // Read and recursively resolve includes in the included file
        let included_content = fs::read_to_string(&canonical)
            .with_context(|| format!("Failed to read included file: {}", canonical.display()))?;
        
        let resolved_content = resolve_includes(&included_content, &canonical, visited)?;
        
        // Replace the !include tag with the resolved content
        // We need to handle indentation - get the indentation of the !include line
        let start = full_match.start();
        let line_start = result[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let indent = &result[line_start..start];
        let indent = indent.chars().take_while(|c| c.is_whitespace()).collect::<String>();
        
        // Indent the included content to match
        let indented_content = if indent.is_empty() {
            resolved_content.trim().to_string()
        } else {
            resolved_content
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        line.to_string() // First line doesn't need extra indent
                    } else if line.is_empty() {
                        String::new()
                    } else {
                        format!("{}{}", indent, line)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        
        result.replace_range(full_match.range(), &indented_content);
        
        visited.remove(&canonical);
    }
    
    Ok(result)
}

/// Check if content has !include tags
pub fn has_includes(content: &str) -> bool {
    content.contains("!include")
}

pub fn convert_file(
    input: &Path,
    output: &Path,
    attribute_strategy: AttributeStrategy,
    known_attributes: &HashSet<String>,
) -> Result<()> {
    info!(input = %input.display(), output = %output.display(), "Converting file");

    let content = fs::read_to_string(input)?;
    
    // Resolve !include tags
    let mut visited = HashSet::new();
    let canonical_input = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());
    visited.insert(canonical_input.clone());
    let resolved_content = resolve_includes(&content, input, &mut visited)?;
    
    let yaml_data: Value = serde_yaml::from_str(&resolved_content)?;

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    if let Some(wix) = yaml_data.get("Wix") {
        convert_element(
            &mut writer,
            "Wix",
            wix,
            attribute_strategy,
            known_attributes,
        )?
    } else if let Value::Mapping(map) = &yaml_data {
        if map.len() == 1 {
            let (key, value) = map.iter().next().unwrap();
            convert_element(
                &mut writer,
                key.as_str().unwrap(),
                value,
                attribute_strategy,
                known_attributes,
            )?
        } else {
            // If multiple roots or flat structure, wrap in Wix?
            // The Python script iterates and appends to a created Wix element
            // But for now let's assume valid structure
            return Err(anyhow::anyhow!("Invalid root structure"));
        }
    } else {
        return Err(anyhow::anyhow!("Invalid YAML structure"));
    };

    let result = writer.into_inner().into_inner();
    let s = String::from_utf8(result)?;

    // Post-process to restore CPack variables
    // Include the leading space in search to remove it during replacement, restoring original no-space format
    let s = s.replace(CPACK_VAR_XML, CPACK_VAR_TOKEN);

    fs::write(output, s)?;

    info!(file = %output.display(), "Successfully converted");
    Ok(())
}

pub fn convert_directory(
    input_dir: &Path,
    output_dir: &Path,
    attribute_strategy: AttributeStrategy,
    known_attributes: &HashSet<String>,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let mut count = 0;
    for entry in fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && (ext == "yaml" || ext == "yml")
        {
            let filename = path.file_stem().unwrap().to_string_lossy();
            let output_path = output_dir.join(format!("{}.wxs", filename));
            convert_file(&path, &output_path, attribute_strategy, known_attributes)?;
            count += 1;
        }
    }

    info!(count = count, "Conversion complete");
    Ok(())
}

fn convert_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag_name: &str,
    data: &Value,
    strategy: AttributeStrategy,
    known_attributes: &HashSet<String>,
) -> Result<()> {
    match data {
        Value::Mapping(map) => {
            let mut elem = BytesStart::new(tag_name);
            let mut children = Vec::new();
            let mut text_content = None;

            // Handle comment if present
            if let Some(comment) = map.get(COMMENT_PROPERTY)
                && let Value::String(s) = comment
            {
                writer.write_event(Event::Comment(BytesText::new(s)))?;
            }

            // Collect attributes
            for (k, v) in map {
                let key = k.as_str().unwrap_or_default();

                if key == COMMENT_PROPERTY {
                    continue;
                }

                let is_attribute = match strategy {
                    AttributeStrategy::Prefix => key.starts_with('@'),
                    AttributeStrategy::Separate => false, // handled differently
                    AttributeStrategy::Inline => {
                        (key.starts_with("xmlns") || known_attributes.contains(key) || key == CPACK_VAR_KEY)
                            && !v.is_mapping()
                            && !v.is_sequence()
                    }
                };

                let is_text = TEXT_PROPERTIES.contains(&key);

                if is_attribute {
                    let attr_name = if strategy == AttributeStrategy::Prefix {
                        &key[1..]
                    } else {
                        key
                    };

                    if let Value::String(s) = v {
                        elem.push_attribute((attr_name, s.as_str()));
                    } else if let Value::Number(n) = v {
                        elem.push_attribute((attr_name, n.to_string().as_str()));
                    } else if let Value::Bool(b) = v {
                        elem.push_attribute((attr_name, if *b { "yes" } else { "no" }));
                    }
                } else if is_text {
                    if let Value::String(s) = v {
                        text_content = Some(s);
                    }
                } else if strategy == AttributeStrategy::Separate && key == ELEMENTS_PROPERTY {
                    match v {
                        Value::Mapping(child_map) => {
                            for (child_key, child_value) in child_map {
                                if let Some(child_name) = child_key.as_str() {
                                    children.push((child_name, child_value));
                                }
                            }
                        }
                        Value::Sequence(seq) => {
                            for item in seq {
                                if let Value::Mapping(child_map) = item {
                                    for (child_key, child_value) in child_map {
                                        if let Some(child_name) = child_key.as_str() {
                                            children.push((child_name, child_value));
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    children.push((key, v));
                }
            }

            // Separate strategy handling
            if strategy == AttributeStrategy::Separate
                && let Some(attrs) = map.get(ATTRIBUTES_PROPERTY)
                && let Value::Mapping(attr_map) = attrs
            {
                for (k, v) in attr_map {
                    let key = k.as_str().unwrap_or_default();
                    if let Value::String(s) = v {
                        elem.push_attribute((key, s.as_str()));
                    } else if let Value::Number(n) = v {
                        elem.push_attribute((key, n.to_string().as_str()));
                    } else if let Value::Bool(b) = v {
                        elem.push_attribute((key, if *b { "yes" } else { "no" }));
                    }
                }
            }

            writer.write_event(Event::Start(elem))?;

            if let Some(text) = text_content {
                writer.write_event(Event::Text(BytesText::new(text)))?;
            }

            for (child_name, child_value) in children {
                if let Value::Sequence(seq) = child_value {
                    for item in seq {
                        convert_element(writer, child_name, item, strategy, known_attributes)?;
                    }
                } else {
                    convert_element(writer, child_name, child_value, strategy, known_attributes)?;
                }
            }

            writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
        }
        Value::Sequence(seq) => {
            // Should not happen at this level if called correctly
            for item in seq {
                convert_element(writer, tag_name, item, strategy, known_attributes)?;
            }
        }
        Value::String(s) => {
            let elem = BytesStart::new(tag_name);
            writer.write_event(Event::Start(elem))?;
            writer.write_event(Event::Text(BytesText::new(s)))?;
            writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wixer_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup_temp_dir(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_resolve_includes() {
        let temp_dir = create_temp_dir();
        
        // Create included file
        let include_path = temp_dir.join("included.yaml");
        {
            let mut include_file = fs::File::create(&include_path).unwrap();
            writeln!(include_file, "Property:").unwrap();
            writeln!(include_file, "  Id: TEST").unwrap();
        } // File handle dropped here
        
        // Create main file with !include
        let main_path = temp_dir.join("main.yaml");
        {
            let mut main_file = fs::File::create(&main_path).unwrap();
            writeln!(main_file, "Wix:").unwrap();
            writeln!(main_file, "  Package: !include included.yaml").unwrap();
        } // File handle dropped here
        
        let content = fs::read_to_string(&main_path).unwrap();
        let mut visited = HashSet::new();
        visited.insert(main_path.canonicalize().unwrap());
        
        let resolved = resolve_includes(&content, &main_path, &mut visited).unwrap();
        
        assert!(resolved.contains("Property:"));
        assert!(resolved.contains("Id: TEST"));
        assert!(!resolved.contains("!include"));
        
        cleanup_temp_dir(&temp_dir);
    }

    #[test]
    fn test_has_includes() {
        assert!(has_includes("foo: !include bar.yaml"));
        assert!(has_includes("  _include: !include test.wxi.yaml"));
        assert!(!has_includes("foo: bar"));
    }

    #[test]
    fn test_circular_include_detection() {
        let temp_dir = create_temp_dir();
        
        // Create file that includes itself
        let self_include_path = temp_dir.join("self.yaml");
        {
            let mut self_file = fs::File::create(&self_include_path).unwrap();
            writeln!(self_file, "Wix: !include self.yaml").unwrap();
        } // File handle dropped here
        
        let content = fs::read_to_string(&self_include_path).unwrap();
        let mut visited = HashSet::new();
        visited.insert(self_include_path.canonicalize().unwrap());
        
        let result = resolve_includes(&content, &self_include_path, &mut visited);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular include"));
        
        cleanup_temp_dir(&temp_dir);
    }
}

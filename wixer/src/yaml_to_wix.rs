use crate::common::AttributeStrategy;
use anyhow::Result;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use serde_yaml::Value;
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tracing::info;

const TEXT_PROPERTIES: &[&str] = &["_value", "value", "text"];
const COMMENT_PROPERTY: &str = "_comment";
const CPACK_VAR_KEY: &str = "CPACK_WIX_CUSTOM_XMLNS_EXPANDED";

pub fn convert_file(
    input: &Path,
    output: &Path,
    attribute_strategy: AttributeStrategy,
    known_attributes: &HashSet<String>,
) -> Result<()> {
    info!(input = %input.display(), output = %output.display(), "Converting file");

    let content = fs::read_to_string(input)?;
    let yaml_data: Value = serde_yaml::from_str(&content)?;

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
    let s = s.replace(" CPACK_WIX_CUSTOM_XMLNS_EXPANDED=\"1\"", "@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@");

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
                } else if strategy == AttributeStrategy::Separate && key == "elements" {
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
            if strategy == AttributeStrategy::Separate {
                if let Some(attrs) = map.get("attributes")
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

use crate::common::AttributeStrategy;
use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tracing::info;

pub fn convert_xml_str_to_yaml_str(
    xml_content: &str,
    attribute_strategy: AttributeStrategy,
) -> Result<String> {
    // Pre-process to handle CPack variables that break XML validity
    let re = Regex::new(r"(@[A-Z_]+@)").unwrap();
    let processed_content = re.replace_all(xml_content, |caps: &regex::Captures| {
        let var = &caps[0];
        if var == "@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@" {
            " CPACK_WIX_CUSTOM_XMLNS_EXPANDED=\"1\"".to_string()
        } else {
            var.to_string()
        }
    });

    let root_val = parse_xml_to_json(&processed_content, attribute_strategy)?;
    let yaml = serde_yaml::to_string(&root_val)?;
    Ok(yaml)
}

pub fn convert_file(
    input: &Path,
    output: &Path,
    attribute_strategy: AttributeStrategy,
    _known_attributes: &HashSet<String>,
) -> Result<()> {
    info!(
        input = %input.display(),
        output = %output.display(),
        "Converting WiX to YAML"
    );

    let content = fs::read_to_string(input)?;
    let yaml = convert_xml_str_to_yaml_str(&content, attribute_strategy)?;
    fs::write(output, yaml)?;

    info!(file = %output.display(), "Successfully converted");
    Ok(())
}

fn parse_xml_to_json(xml_content: &str, strategy: AttributeStrategy) -> Result<Value> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<(String, Map<String, Value>)> = Vec::new();
    let mut root_map: Option<Map<String, Value>> = None;
    let mut pending_comment: Option<String> = None;

    // Helper to add child to parent
    fn add_child(parent: &mut Map<String, Value>, name: String, child: Value) {
        if let Some(existing) = parent.get_mut(&name) {
            if let Value::Array(arr) = existing {
                arr.push(child);
            } else {
                let old_val = existing.take();
                parent.insert(name, Value::Array(vec![old_val, child]));
            }
        } else {
            parent.insert(name, child);
        }
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec())?;
                let mut map = Map::new();

                // Attach pending comment
                if let Some(comment) = pending_comment.take() {
                    map.insert("_comment".to_string(), Value::String(comment));
                }

                for attr in e.attributes() {
                    let attr = attr?;
                    let key = String::from_utf8(attr.key.as_ref().to_vec())?;
                    let val_str = String::from_utf8(attr.value.as_ref().to_vec())?;

                    let prop_name = match strategy {
                        AttributeStrategy::Prefix => format!("@{}", key),
                        _ => key,
                    };

                    let val = Value::String(val_str);
                    map.insert(prop_name, val);
                }

                stack.push((name, map));
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec())?;
                let mut map = Map::new();

                // Attach pending comment
                if let Some(comment) = pending_comment.take() {
                    map.insert("_comment".to_string(), Value::String(comment));
                }

                for attr in e.attributes() {
                    let attr = attr?;
                    let key = String::from_utf8(attr.key.as_ref().to_vec())?;
                    let val_str = String::from_utf8(attr.value.as_ref().to_vec())?;

                    let prop_name = match strategy {
                        AttributeStrategy::Prefix => format!("@{}", key),
                        _ => key,
                    };

                    let val = Value::String(val_str);
                    map.insert(prop_name, val);
                }

                if let Some((_, parent_map)) = stack.last_mut() {
                    add_child(parent_map, name, Value::Object(map));
                } else if root_map.is_none() {
                    // Root empty element
                    let mut root = Map::new();
                    root.insert(name, Value::Object(map));
                    root_map = Some(root);
                }
            }
            Ok(Event::Comment(e)) => {
                let comment = e.unescape()?.to_string();
                pending_comment = Some(comment);
            }
            Ok(Event::PI(e)) => {
                // Handle processing instructions like <?include filename?>
                let pi_content = String::from_utf8(e.as_ref().to_vec())?;
                if pi_content.starts_with("include ") {
                    let include_path = pi_content.trim_start_matches("include ").trim();
                    // Convert to !include syntax
                    // Change extension from .wxi to .wxi.yaml if it's a WiX include
                    let yaml_path =
                        if include_path.ends_with(".wxi") || include_path.ends_with(".wxs") {
                            format!("{}.yaml", include_path)
                        } else {
                            include_path.to_string()
                        };

                    if let Some((_, parent_map)) = stack.last_mut() {
                        // Store as special _include directive
                        // Format: "!include path" which yaml_to_wix will recognize
                        parent_map.insert(
                            "_include".to_string(),
                            Value::String(format!("!include {}", yaml_path)),
                        );
                    }
                }
                // Ignore other PIs like <?xml version="1.0"?>
            }
            Ok(Event::End(_)) => {
                if let Some((name, map)) = stack.pop() {
                    if let Some((_, parent_map)) = stack.last_mut() {
                        add_child(parent_map, name, Value::Object(map));
                    } else {
                        // Root element closing
                        let mut root = Map::new();
                        root.insert(name, Value::Object(map));
                        root_map = Some(root);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape()?.to_string();
                if !text.trim().is_empty()
                    && let Some((_, map)) = stack.last_mut()
                {
                    // Determine text property name based on strategy/convention
                    // For now using "_value" as seen in yaml_to_wix or "text"
                    // yaml_to_wix uses: const TEXT_PROPERTIES: &[&str] = &["_value", "value", "text"];
                    // Let's use _value to be safe/standard
                    map.insert("_value".to_string(), Value::String(text));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error at position {}: {:?}",
                    reader.buffer_position(),
                    e
                ));
            }
            _ => (),
        }
        buf.clear();
    }

    Ok(Value::Object(root_map.unwrap_or_default()))
}

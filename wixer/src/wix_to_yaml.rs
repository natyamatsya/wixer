use crate::common::AttributeStrategy;
use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::{Map, Value};
use serde_yaml;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tracing::info;

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
    let root_val = parse_xml_to_json(&content, attribute_strategy)?;

    let file = fs::File::create(output)?;
    serde_yaml::to_writer(file, &root_val)?;

    info!(file = %output.display(), "Successfully converted");
    Ok(())
}

fn parse_xml_to_json(xml_content: &str, strategy: AttributeStrategy) -> Result<Value> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<(String, Map<String, Value>)> = Vec::new();
    let mut root_map: Option<Map<String, Value>> = None;

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

                for attr in e.attributes() {
                    let attr = attr?;
                    let key = String::from_utf8(attr.key.as_ref().to_vec())?;
                    let val_str = String::from_utf8(attr.value.as_ref().to_vec())?;

                    let prop_name = match strategy {
                        AttributeStrategy::Prefix => format!("@{}", key),
                        _ => key,
                    };

                    let val = if val_str == "yes" {
                        Value::Bool(true)
                    } else if val_str == "no" {
                        Value::Bool(false)
                    } else {
                        Value::String(val_str)
                    };
                    map.insert(prop_name, val);
                }

                stack.push((name, map));
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec())?;
                let mut map = Map::new();

                for attr in e.attributes() {
                    let attr = attr?;
                    let key = String::from_utf8(attr.key.as_ref().to_vec())?;
                    let val_str = String::from_utf8(attr.value.as_ref().to_vec())?;

                    let prop_name = match strategy {
                        AttributeStrategy::Prefix => format!("@{}", key),
                        _ => key,
                    };

                    let val = if val_str == "yes" {
                        Value::Bool(true)
                    } else if val_str == "no" {
                        Value::Bool(false)
                    } else {
                        Value::String(val_str)
                    };
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
                if !text.trim().is_empty() {
                    if let Some((_, map)) = stack.last_mut() {
                        // Determine text property name based on strategy/convention
                        // For now using "_value" as seen in yaml_to_wix or "text"
                        // yaml_to_wix uses: const TEXT_PROPERTIES: &[&str] = &["_value", "value", "text"];
                        // Let's use _value to be safe/standard
                        map.insert("_value".to_string(), Value::String(text));
                    }
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

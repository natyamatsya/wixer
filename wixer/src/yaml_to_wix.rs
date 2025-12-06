use crate::AttributeStrategy;
use anyhow::Result;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use serde_yaml::Value;
use std::fs;
use std::io::Cursor;
use std::path::Path;

const ATTRIBUTE_PROPERTIES: &[&str] = &[
    "Id",
    "Name",
    "Version",
    "Language",
    "Manufacturer",
    "UpgradeCode",
    "InstallerVersion",
    "Compressed",
    "InstallScope",
    "Description",
    "Comments",
    "DowngradeErrorMessage",
    "Schedule",
    "EmbedCab",
    "CompressionLevel",
    "Guid",
    "Win64",
    "Source",
    "KeyPath",
    "Target",
    "WorkingDirectory",
    "Icon",
    "IconIndex",
    "Root",
    "Key",
    "Type",
    "Value",
    "Title",
    "Level",
    "ConfigurableDirectory",
    "SourceFile",
    "Arguments",
    "On",
    "Directory",
    "xmlns",
    "Scope",
];

const TEXT_PROPERTIES: &[&str] = &["_value", "value", "text"];

pub fn convert_file(
    input: &Path,
    output: &Path,
    attribute_strategy: AttributeStrategy,
) -> Result<()> {
    println!("Converting {} -> {}", input.display(), output.display());

    let content = fs::read_to_string(input)?;
    let yaml_data: Value = serde_yaml::from_str(&content)?;

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    if let Some(wix) = yaml_data.get("Wix") {
        convert_element(&mut writer, "Wix", wix, attribute_strategy)?
    } else if let Value::Mapping(map) = &yaml_data {
        if map.len() == 1 {
            let (key, value) = map.iter().next().unwrap();
            convert_element(
                &mut writer,
                key.as_str().unwrap(),
                value,
                attribute_strategy,
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
    fs::write(output, result)?;

    println!("✓ Successfully converted to {}", output.display());
    Ok(())
}

pub fn convert_directory(
    input_dir: &Path,
    output_dir: &Path,
    attribute_strategy: AttributeStrategy,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let mut count = 0;
    for entry in fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    let filename = path.file_stem().unwrap().to_string_lossy();
                    let output_path = output_dir.join(format!("{}.wxs", filename));
                    convert_file(&path, &output_path, attribute_strategy)?;
                    count += 1;
                }
            }
        }
    }

    println!("\nConversion complete: {} files processed", count);
    Ok(())
}

fn convert_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag_name: &str,
    data: &Value,
    strategy: AttributeStrategy,
) -> Result<()> {
    match data {
        Value::Mapping(map) => {
            let mut elem = BytesStart::new(tag_name);
            let mut children = Vec::new();
            let mut text_content = None;

            // Collect attributes
            for (k, v) in map {
                let key = k.as_str().unwrap_or_default();

                let is_attribute = match strategy {
                    AttributeStrategy::Prefix => key.starts_with('@'),
                    AttributeStrategy::Separate => false, // handled differently
                    AttributeStrategy::Inline => {
                        ATTRIBUTE_PROPERTIES.contains(&key) && !v.is_mapping() && !v.is_sequence()
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
                } else {
                    children.push((key, v));
                }
            }

            // Separate strategy handling
            if strategy == AttributeStrategy::Separate {
                if let Some(attrs) = map.get("attributes") {
                    if let Value::Mapping(attr_map) = attrs {
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
                if let Some(elems) = map.get("elements") {
                    // TODO: Handle separate elements
                }
            }

            writer.write_event(Event::Start(elem))?;

            if let Some(text) = text_content {
                writer.write_event(Event::Text(BytesText::new(text)))?;
            }

            for (child_name, child_value) in children {
                if let Value::Sequence(seq) = child_value {
                    for item in seq {
                        convert_element(writer, child_name, item, strategy)?;
                    }
                } else {
                    convert_element(writer, child_name, child_value, strategy)?;
                }
            }

            writer.write_event(Event::End(BytesEnd::new(tag_name)))?;
        }
        Value::Sequence(seq) => {
            // Should not happen at this level if called correctly
            for item in seq {
                convert_element(writer, tag_name, item, strategy)?;
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

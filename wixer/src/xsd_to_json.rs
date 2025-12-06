use crate::common::AttributeStrategy;
use anyhow::{Context, Result};
use roxmltree::{Document, Node};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub async fn download_wix_schemas(output_dir: &Path, force: bool) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(output_dir)?;

    let base_url = "https://raw.githubusercontent.com/wixtoolset/web/master/src/xsd4/";
    let schemas = [
        "wix.xsd",
        "wixloc.xsd",
        "bal.xsd",
        "complus.xsd",
        "dependency.xsd",
        "difxapp.xsd",
        "firewall.xsd",
        "http.xsd",
        "iis.xsd",
        "netfx.xsd",
        "ps.xsd",
        "sql.xsd",
        "util.xsd",
        "vs.xsd",
    ];

    info!(dir = %output_dir.display(), "Downloading WiX v4 schemas");
    let client = reqwest::Client::new();
    let mut downloaded_files = Vec::new();

    for schema_path in schemas {
        let url = format!("{}{}", base_url, schema_path);
        let output_path = output_dir.join(schema_path);

        if output_path.exists() && !force {
            info!(schema = %schema_path, "Using cached schema");
            downloaded_files.push(output_path);
            continue;
        }

        info!(schema = %schema_path, "Downloading schema");
        let content = client.get(&url).send().await?.text().await?;
        fs::write(&output_path, content)?;
        downloaded_files.push(output_path);
    }

    info!(count = schemas.len(), "Downloaded/Verified schema files");
    Ok(downloaded_files)
}

pub async fn download_msix_schemas(output_dir: &Path, force: bool) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(output_dir)?;

    let client = reqwest::Client::builder().user_agent("wixer").build()?;

    let base_api = "https://api.github.com/repos/microsoft/msix-packaging/contents/resources/AppxPackaging/Manifest/Schema";

    info!(dir = %output_dir.display(), "Fetching MSIX schema list...");

    let resp = client.get(base_api).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to fetch MSIX schema list: {}", resp.status());
    }

    let contents: Vec<Value> = resp.json().await?;
    let mut downloaded_files = Vec::new();

    for item in contents {
        if item["type"] == "dir" {
            let name = item["name"].as_str().unwrap();
            // Filter for year-like directories to avoid grabbing unrelated folders if any appear
            if !name.chars().all(char::is_numeric) {
                continue;
            }

            let year_dir = output_dir.join(name);
            fs::create_dir_all(&year_dir)?;

            let dir_url = item["url"].as_str().unwrap();
            let resp = client.get(dir_url).send().await?;
            if !resp.status().is_success() {
                warn!(
                    "Failed to fetch schema list for {}: {}",
                    name,
                    resp.status()
                );
                continue;
            }

            let dir_contents: Vec<Value> = resp.json().await?;
            for file_item in dir_contents {
                if file_item["type"] == "file" {
                    let file_name = file_item["name"].as_str().unwrap();
                    if file_name.ends_with(".xsd") {
                        let download_url = file_item["download_url"].as_str().unwrap();
                        let output_path = year_dir.join(file_name);

                        if output_path.exists() && !force {
                            downloaded_files.push(output_path);
                            continue;
                        }

                        info!(schema = %file_name, "Downloading MSIX schema");
                        let content = client.get(download_url).send().await?.text().await?;
                        fs::write(&output_path, content)?;
                        downloaded_files.push(output_path);
                    }
                }
            }
        }
    }

    info!(
        count = downloaded_files.len(),
        "Downloaded/Verified MSIX schema files"
    );
    Ok(downloaded_files)
}

pub fn convert_xsd(
    input: &str,
    output: Option<&str>,
    output_dir: &Path,
    attribute_strategy: AttributeStrategy,
    include_descriptions: bool,
    strict_mode: bool,
    root_element: Option<&str>,
) -> Result<HashSet<String>> {
    fs::create_dir_all(output_dir)?;

    // TODO: Handle URL input
    let input_path = Path::new(input);
    let content = fs::read_to_string(input_path)
        .with_context(|| format!("Failed to read input file: {}", input))?;

    // Calculate hash
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let current_hash = hex::encode(hasher.finalize());

    let output_path = if let Some(out) = output {
        PathBuf::from(out)
    } else {
        let filename = input_path.file_stem().unwrap().to_string_lossy();
        output_dir.join(format!("{}.schema.json", filename))
    };

    let hash_path = output_path.with_extension("sha256");
    let attributes_path = output_path.with_extension("attributes.json");

    // Check cache
    if output_path.exists() && hash_path.exists() && attributes_path.exists() {
        if let Ok(stored_hash) = fs::read_to_string(&hash_path) {
            if stored_hash.trim() == current_hash {
                if let Ok(attr_file) = fs::File::open(&attributes_path) {
                    if let Ok(cached_attrs) = serde_json::from_reader(attr_file) {
                        info!(file = %output_path.display(), "Using cached schema");
                        return Ok(cached_attrs);
                    }
                }
            }
        }
    }

    let converter = XsdConverter::new(attribute_strategy, include_descriptions, strict_mode);

    let json_schema = converter.convert(&content, root_element)?;

    let file = fs::File::create(&output_path)?;
    serde_json::to_writer_pretty(file, &json_schema)?;

    // Save hash
    fs::write(&hash_path, &current_hash)?;

    // Save attributes
    let attributes = converter.attributes.take();
    let attr_file = fs::File::create(&attributes_path)?;
    serde_json::to_writer_pretty(attr_file, &attributes)?;

    info!(file = %output_path.display(), "Generated schema");
    Ok(attributes)
}

struct XsdConverter {
    attribute_strategy: AttributeStrategy,
    include_descriptions: bool,
    strict_mode: bool,
    _xs_namespace: String,
    attributes: RefCell<HashSet<String>>,
}

impl XsdConverter {
    fn new(
        attribute_strategy: AttributeStrategy,
        include_descriptions: bool,
        strict_mode: bool,
    ) -> Self {
        Self {
            attribute_strategy,
            include_descriptions,
            strict_mode,
            _xs_namespace: "http://www.w3.org/2001/XMLSchema".to_string(),
            attributes: RefCell::new(HashSet::new()),
        }
    }

    fn convert(&self, content: &str, root_element: Option<&str>) -> Result<Value> {
        let doc = Document::parse(content)?;
        let root = doc.root_element();

        let mut definitions = Map::new();
        let mut properties = Map::new();

        // Extract Types
        for node in root.children() {
            if node.tag_name().name() == "complexType" && node.is_element() {
                if let Some(name) = node.attribute("name") {
                    definitions.insert(name.to_string(), self.convert_complex_type(node));
                }
            } else if node.tag_name().name() == "simpleType"
                && node.is_element()
                && let Some(name) = node.attribute("name")
            {
                definitions.insert(name.to_string(), self.convert_simple_type(node));
            }
        }

        // Extract Elements
        for node in root.children() {
            if node.tag_name().name() == "element"
                && node.is_element()
                && let Some(name) = node.attribute("name")
            {
                let schema = self.convert_element(node);
                properties.insert(name.to_string(), schema.clone());
                definitions.insert(name.to_string(), schema);
            }
        }

        let mut schema = json!({
            "$schema": "https://json-schema.org/draft-07/schema#",
            "type": "object",
            "definitions": definitions,
        });

        if let Some(root_name) = root_element {
            // Find root element definition and use it
            // Logic to be improved to properly link root element
            if let Some(prop) = properties.get(root_name) {
                schema
                    .as_object_mut()
                    .unwrap()
                    .extend(prop.as_object().unwrap().clone());
            } else {
                warn!(root_element = %root_name, "Root element not found");
            }
        } else {
            schema
                .as_object_mut()
                .unwrap()
                .insert("properties".to_string(), Value::Object(properties));
        }

        Ok(schema)
    }

    fn convert_complex_type(&self, node: Node) -> Value {
        let mut schema = Map::new();
        schema.insert("type".to_string(), json!("object"));
        schema.insert("properties".to_string(), json!({}));
        schema.insert("additionalProperties".to_string(), json!(false));

        if self.include_descriptions
            && let Some(desc) = self.get_annotation(node)
        {
            schema.insert("description".to_string(), json!(desc));
        }

        let mut properties = Map::new();
        let mut required = Vec::new();

        // Attributes
        for child in node.descendants() {
            if child.tag_name().name() == "attribute" && child.is_element() {
                // Check if this attribute is a direct child or inside complexContent/extension...
                // Simplified traversal for now
                let name = child.attribute("name");
                if let Some(attr_name) = name {
                    // Record attribute name
                    self.attributes.borrow_mut().insert(attr_name.to_string());

                    let prop_name = match self.attribute_strategy {
                        AttributeStrategy::Prefix => format!("@{}", attr_name),
                        _ => attr_name.to_string(),
                    };

                    properties.insert(prop_name.clone(), self.convert_attribute(child));

                    if self.strict_mode && child.attribute("use") == Some("required") {
                        required.push(prop_name);
                    }
                }
            }
        }

        // Check for anyAttribute or any element
        let has_any_attribute = node
            .descendants()
            .any(|n| n.tag_name().name() == "anyAttribute");
        let has_any_element = node.descendants().any(|n| n.tag_name().name() == "any");

        if has_any_attribute || has_any_element {
            schema.insert("additionalProperties".to_string(), json!(true));
        }

        // Elements (sequence, choice, all)
        for child in node.children() {
            match child.tag_name().name() {
                "sequence" | "choice" | "all" | "complexContent" => {
                    self.process_content_model(child, &mut properties, &mut required);
                }
                _ => {}
            }
        }

        // For simpleContent (extensions)
        if let Some(simple_content) = node
            .children()
            .find(|n| n.tag_name().name() == "simpleContent")
            && let Some(extension) = simple_content
                .children()
                .find(|n| n.tag_name().name() == "extension")
        {
            // Handle attributes in extension
            for child in extension.descendants() {
                if child.tag_name().name() == "attribute" && child.is_element() {
                    let name = child.attribute("name");
                    if let Some(attr_name) = name {
                        // Record attribute name
                        self.attributes.borrow_mut().insert(attr_name.to_string());

                        let prop_name = match self.attribute_strategy {
                            AttributeStrategy::Prefix => format!("@{}", attr_name),
                            _ => attr_name.to_string(),
                        };
                        properties.insert(prop_name.clone(), self.convert_attribute(child));
                        if self.strict_mode && child.attribute("use") == Some("required") {
                            required.push(prop_name);
                        }
                    }
                }
            }
            // Add value property
            if let Some(base) = extension.attribute("base") {
                let (json_type, _) = self.map_xsd_type(base);
                let value_prop = match self.attribute_strategy {
                    AttributeStrategy::Inline => "_value",
                    _ => "value",
                };
                properties.insert(value_prop.to_string(), json!({"type": json_type}));
            }
        }

        // For complexContent (extensions)
        if let Some(complex_content) = node
            .children()
            .find(|n| n.tag_name().name() == "complexContent")
            && let Some(extension) = complex_content
                .children()
                .find(|n| n.tag_name().name() == "extension")
        {
            // Handle base type properties - theoretically we should merge them,
            // but for now we just handle the local additions
            for child in extension.children() {
                match child.tag_name().name() {
                    "sequence" | "choice" | "all" => {
                        self.process_content_model(child, &mut properties, &mut required);
                    }
                    _ => {}
                }
            }
            // Handle attributes in extension
            for child in extension.descendants() {
                if child.tag_name().name() == "attribute" && child.is_element() {
                    let name = child.attribute("name");
                    if let Some(attr_name) = name {
                        // Record attribute name
                        self.attributes.borrow_mut().insert(attr_name.to_string());

                        let prop_name = match self.attribute_strategy {
                            AttributeStrategy::Prefix => format!("@{}", attr_name),
                            _ => attr_name.to_string(),
                        };
                        properties.insert(prop_name.clone(), self.convert_attribute(child));
                        if self.strict_mode && child.attribute("use") == Some("required") {
                            required.push(prop_name);
                        }
                    }
                }
            }
        }

        // For now, let's add properties to schema
        schema.insert("properties".to_string(), Value::Object(properties));

        if !required.is_empty() {
            schema.insert("required".to_string(), json!(required));
        }

        Value::Object(schema)
    }

    fn process_content_model(
        &self,
        node: Node,
        properties: &mut Map<String, Value>,
        required: &mut Vec<String>,
    ) {
        // Check maxOccurs of the container itself
        let container_max_occurs = node.attribute("maxOccurs").unwrap_or("1");
        let container_is_array = container_max_occurs == "unbounded"
            || (container_max_occurs.parse::<u32>().unwrap_or(1) > 1);

        for child in node.children() {
            if child.tag_name().name() == "element" {
                if let Some(ref_name) = child.attribute("ref") {
                    let local_name = ref_name.split(':').next_back().unwrap_or(ref_name);

                    let max_occurs = child.attribute("maxOccurs").unwrap_or("1");
                    let min_occurs = child.attribute("minOccurs").unwrap_or("1");

                    // Determine if it should be an array:
                    // 1. Element itself has maxOccurs > 1
                    // 2. Parent container has maxOccurs > 1 (inheritance)
                    let is_array = max_occurs == "unbounded"
                        || (max_occurs.parse::<u32>().unwrap_or(1) > 1)
                        || container_is_array;

                    let mut property_schema = json!({
                        "$ref": format!("#/definitions/{}", local_name)
                    });

                    if is_array {
                        property_schema = json!({
                            "oneOf": [
                                {
                                    "type": "array",
                                    "items": property_schema
                                },
                                property_schema
                            ]
                        });
                    }

                    if let Some(existing) = properties.get(&local_name.to_string()) {
                        // Combine with existing schema using oneOf
                        let new_schema = json!({
                            "oneOf": [
                                existing,
                                property_schema
                            ]
                        });
                        properties.insert(local_name.to_string(), new_schema);
                    } else {
                        properties.insert(local_name.to_string(), property_schema);
                    }

                    if self.strict_mode && min_occurs != "0" && !container_is_array {
                        required.push(local_name.to_string());
                    }
                } else if let Some(name) = child.attribute("name") {
                    let mut elem_schema = self.convert_element(child);

                    let max_occurs = child.attribute("maxOccurs").unwrap_or("1");
                    let _min_occurs = child.attribute("minOccurs").unwrap_or("1");

                    let is_array = max_occurs == "unbounded"
                        || (max_occurs.parse::<u32>().unwrap_or(1) > 1)
                        || container_is_array;

                    if is_array {
                        elem_schema = json!({
                            "oneOf": [
                                {
                                    "type": "array",
                                    "items": elem_schema
                                },
                                elem_schema
                            ]
                        });
                    }

                    if let Some(existing) = properties.get(&name.to_string()) {
                        let new_schema = json!({
                            "oneOf": [
                                existing,
                                elem_schema
                            ]
                        });
                        properties.insert(name.to_string(), new_schema);
                    } else {
                        properties.insert(name.to_string(), elem_schema);
                    }
                    self.process_content_model(child, properties, required);
                } else if child.tag_name().name() == "group" {
                    // Handle group ref
                }
            }
        }
    }

    fn convert_simple_type(&self, node: Node) -> Value {
        let mut schema = Map::new();

        if self.include_descriptions
            && let Some(desc) = self.get_annotation(node)
        {
            schema.insert("description".to_string(), json!(desc));
        }

        if let Some(restriction) = node
            .children()
            .find(|n| n.tag_name().name() == "restriction")
        {
            if let Some(base) = restriction.attribute("base") {
                let (json_types, pattern) = self.map_xsd_type(base);
                if json_types.len() == 1 {
                    schema.insert("type".to_string(), json_types[0].clone());
                } else {
                    schema.insert("type".to_string(), Value::Array(json_types.clone()));
                }

                if let Some(p) = pattern {
                    // If pattern applies to string type, we need to be careful if type is array [integer, string]
                    // But 'pattern' keyword applies to strings.
                    schema.insert("pattern".to_string(), json!(p));
                }
            }

            let mut enums = Vec::new();
            for child in restriction.children() {
                if child.tag_name().name() == "enumeration"
                    && let Some(val) = child.attribute("value")
                {
                    enums.push(json!(val));
                }
            }
            if !enums.is_empty() {
                schema.insert("enum".to_string(), Value::Array(enums));
            }
        } else {
            schema.insert("type".to_string(), json!("string"));
        }

        Value::Object(schema)
    }

    fn convert_element(&self, node: Node) -> Value {
        let mut schema = Map::new();

        if self.include_descriptions
            && let Some(desc) = self.get_annotation(node)
        {
            schema.insert("description".to_string(), json!(desc));
        }

        if let Some(type_name) = node.attribute("type") {
            // Reference to existing type
            // If it's a built-in type
            if type_name.starts_with("xs:") || type_name.starts_with("xsd:") {
                let (json_types, _) = self.map_xsd_type(type_name);
                if json_types.len() == 1 {
                    schema.insert("type".to_string(), json_types[0].clone());
                } else {
                    schema.insert("type".to_string(), Value::Array(json_types));
                }
            } else {
                // Reference to custom definition
                let local_name = type_name.split(':').next_back().unwrap_or(type_name);
                schema.insert(
                    "$ref".to_string(),
                    json!(format!("#/definitions/{}", local_name)),
                );
            }
        } else {
            // Inline type definition
            if let Some(complex) = node
                .children()
                .find(|n| n.tag_name().name() == "complexType")
            {
                let complex_schema = self.convert_complex_type(complex);
                return complex_schema;
            } else if let Some(simple) = node
                .children()
                .find(|n| n.tag_name().name() == "simpleType")
            {
                let simple_schema = self.convert_simple_type(simple);
                return simple_schema;
            } else {
                schema.insert("type".to_string(), json!("string"));
            }
        }

        Value::Object(schema)
    }

    fn convert_attribute(&self, node: Node) -> Value {
        let mut schema = Map::new();

        if self.include_descriptions
            && let Some(desc) = self.get_annotation(node)
        {
            schema.insert("description".to_string(), json!(desc));
        }

        if let Some(type_name) = node.attribute("type") {
            let (json_types, pattern) = self.map_xsd_type(type_name);
            if json_types.len() == 1 {
                schema.insert("type".to_string(), json_types[0].clone());
            } else {
                schema.insert("type".to_string(), Value::Array(json_types.clone()));
            }
            if let Some(p) = pattern {
                schema.insert("pattern".to_string(), json!(p));
            }
        } else {
            schema.insert("type".to_string(), json!("string"));
        }

        Value::Object(schema)
    }

    fn get_annotation(&self, node: Node) -> Option<String> {
        node.children()
            .find(|n| n.tag_name().name() == "annotation")
            .and_then(|n| {
                n.children()
                    .find(|c| c.tag_name().name() == "documentation")
            })
            .and_then(|n| n.text().map(|t| t.trim().to_string()))
    }

    fn map_xsd_type(&self, xsd_type: &str) -> (Vec<Value>, Option<&'static str>) {
        let local_name = xsd_type.split(':').next_back().unwrap_or(xsd_type);
        match local_name {
            "string" | "normalizedString" | "token" | "NCName" | "NMTOKEN" | "anyURI"
            | "language" | "ID" | "IDREF" => (vec![json!("string")], None),
            "boolean" => (vec![json!("boolean"), json!("string")], None),
            "integer" | "int" | "long" | "short" | "byte" | "nonNegativeInteger"
            | "positiveInteger" | "unsignedLong" | "unsignedInt" | "unsignedShort"
            | "unsignedByte" | "Integer" | "LocalizableInteger" => {
                (vec![json!("integer"), json!("string")], Some("^-?[0-9]+$"))
            }
            "decimal" | "float" | "double" => (
                vec![json!("number"), json!("string")],
                Some("^-?[0-9]*\\.?[0-9]+$"),
            ),
            "Guid" | "AutogenGuid" | "ComponentGuid" => (
                vec![json!("string")],
                Some(
                    "^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$|\\*|PUT-GUID-HERE",
                ),
            ),
            _ => (vec![json!("string")], None), // Default to string
        }
    }
}

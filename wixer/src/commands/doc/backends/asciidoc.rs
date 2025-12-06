use crate::commands::doc::generator::{DocGenerator, IndexContext};
use crate::commands::doc::mermaid::generate_element_mermaid;
use crate::commands::doc::parser::{extract_documentation, get_attributes, get_child_elements};
use anyhow::Result;
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct AsciiDocGenerator;

impl DocGenerator for AsciiDocGenerator {
    fn generate_index(&self, output_dir: &Path, context: &IndexContext) -> Result<()> {
        let mut adoc = String::from(
            "= WiX Toolset v4 Schema Documentation\n:toc:\n\nComprehensive reference for WiX v4 XML schemas\n\n",
        );

        adoc.push_str("== Schema Files\n\n[cols=\"1,1,2\",options=\"header\"]\n|===\n|Schema |Namespace |Description\n");

        let mut sorted_schemas: Vec<_> = context.schemas.keys().collect();
        sorted_schemas.sort();

        for name in sorted_schemas {
            if let Some(info) = context.schemas.get(name) {
                let doc = info
                    .documentation
                    .as_deref()
                    .unwrap_or("")
                    .split('.')
                    .next()
                    .unwrap_or("");
                let doc_clean = doc.replace('\n', " ").trim().to_string();
                adoc.push_str(&format!(
                    "|link:schema_{0}.adoc[{0}.xsd] |`{1}` |{2}\n",
                    name, info.target_ns, doc_clean
                ));
            }
        }
        adoc.push_str("|===\n\n");

        adoc.push_str(
            "== All Elements\n\n[cols=\"1,2\",options=\"header\"]\n|===\n|Element |Schema(s)\n",
        );
        let mut sorted_elements: Vec<_> = context.elements.keys().collect();
        sorted_elements.sort();

        for name in sorted_elements {
            if let Some(schemas) = context.elements.get(name) {
                let schema_links = schemas
                    .iter()
                    .map(|s| format!("link:schema_{}.adoc[{}]", s, s))
                    .collect::<Vec<_>>()
                    .join(", ");
                adoc.push_str(&format!(
                    "|link:element_{0}.adoc[{0}] |{1}\n",
                    name, schema_links
                ));
            }
        }
        adoc.push_str("|===\n");

        fs::write(output_dir.join("index.adoc"), adoc)?;
        Ok(())
    }

    fn generate_schema_page(
        &self,
        output_dir: &Path,
        schema_name: &str,
        doc: &Document,
    ) -> Result<()> {
        let root = doc.root_element();
        let target_ns = root.attribute("targetNamespace").unwrap_or("");
        let schema_doc = extract_documentation(root);

        let mut adoc = format!(
            "= {}.xsd\n:toc:\n\n*Namespace:* `{}`\n\n",
            schema_name, target_ns
        );

        if let Some(doc) = schema_doc {
            adoc.push_str("== Description\n\n");
            adoc.push_str(&doc);
            adoc.push_str("\n\n");
        }

        adoc.push_str(
            "== Elements\n\n[cols=\"1,3\",options=\"header\"]\n|===\n|Element |Description\n",
        );

        let mut elements: Vec<Node> = Vec::new();
        for node in doc.descendants() {
            if node.tag_name().name() == "element" && node.has_attribute("name") {
                elements.push(node);
            }
        }

        for node in elements {
            let name = node.attribute("name").unwrap();
            let doc = extract_documentation(node);
            let brief = doc
                .as_deref()
                .unwrap_or("")
                .split('.')
                .next()
                .unwrap_or("")
                .replace('\n', " ")
                .trim()
                .to_string();
            adoc.push_str(&format!("|link:element_{0}.adoc[{0}] |{1}\n", name, brief));
        }
        adoc.push_str("|===\n\nlink:../index.adoc[Back to Index]\n");

        fs::write(
            output_dir.join(format!("schema_{}.adoc", schema_name)),
            adoc,
        )?;
        Ok(())
    }

    fn generate_element_page(
        &self,
        output_dir: &Path,
        schema_name: &str,
        node: Node,
        relationships: &HashMap<String, Vec<String>>,
    ) -> Result<()> {
        let name = node.attribute("name").unwrap();
        let doc = extract_documentation(node);

        let mut adoc = format!("= {}\n:toc:\n\n*Schema:* `{}.xsd`\n\n", name, schema_name);

        if let Some(d) = doc {
            adoc.push_str("== Description\n\n");
            adoc.push_str(&d);
            adoc.push_str("\n\n");
        }

        // Mermaid
        if relationships.contains_key(name) {
            adoc.push_str("== Element Hierarchy\n\n");
            adoc.push_str("[mermaid]\n....\n");
            adoc.push_str(&generate_element_mermaid(name, relationships));
            adoc.push_str("\n....\n\n");
        }

        let attributes = get_attributes(node);
        if !attributes.is_empty() {
            adoc.push_str("== Attributes\n\n[cols=\"1,1,1,3\",options=\"header\"]\n|===\n|Name |Type |Use |Description\n");
            for attr in attributes {
                let use_str = if attr.use_attr == "required" {
                    format!("*{}*", attr.use_attr)
                } else {
                    attr.use_attr
                };
                let doc_str = attr.doc.unwrap_or_default().replace('\n', " ");
                adoc.push_str(&format!(
                    "|`{}` |`{}` |{} |{}\n",
                    attr.name, attr.type_name, use_str, doc_str
                ));
            }
            adoc.push_str("|===\n\n");
        }

        let children = get_child_elements(node);
        if !children.is_empty() {
            adoc.push_str("== Child Elements\n\n[cols=\"1,1,3\",options=\"header\"]\n|===\n|Element |Cardinality |Description\n");
            for child in children {
                let cardinality = format!("{}..{}", child.min_occurs, child.max_occurs);
                let doc_str = child.doc.unwrap_or_default().replace('\n', " ");
                adoc.push_str(&format!(
                    "|link:element_{0}.adoc[{0}] |`{1}` |{2}\n",
                    child.name, cardinality, doc_str
                ));
            }
            adoc.push_str("|===\n\n");
        }

        adoc.push_str(&format!(
            "link:schema_{0}.adoc[Back to Schema] | link:../index.adoc[Back to Index]\n",
            schema_name
        ));

        fs::write(output_dir.join(format!("element_{}.adoc", name)), adoc)?;
        Ok(())
    }
}

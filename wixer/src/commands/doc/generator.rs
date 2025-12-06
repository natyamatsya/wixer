use super::types::SchemaInfo;
use anyhow::Result;
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::path::Path;

pub struct IndexContext<'a> {
    pub schemas: &'a HashMap<String, SchemaInfo>,
    pub elements: &'a HashMap<String, Vec<String>>,
}

pub trait DocGenerator {
    fn generate_index(&self, output_dir: &Path, context: &IndexContext) -> Result<()>;

    fn generate_schema_page(
        &self,
        output_dir: &Path,
        schema_name: &str,
        doc: &Document,
    ) -> Result<()>;

    fn generate_element_page(
        &self,
        output_dir: &Path,
        schema_name: &str,
        node: Node,
        relationships: &HashMap<String, Vec<String>>,
    ) -> Result<()>;
}

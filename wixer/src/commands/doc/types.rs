pub struct SchemaInfo {
    pub target_ns: String,
    pub documentation: Option<String>,
}

pub struct ChildInfo {
    pub name: String,
    pub min_occurs: String,
    pub max_occurs: String,
    pub doc: Option<String>,
}

pub struct AttributeInfo {
    pub name: String,
    pub type_name: String,
    pub use_attr: String,
    pub doc: Option<String>,
}

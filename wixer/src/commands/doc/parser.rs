use super::types::{AttributeInfo, ChildInfo};
use roxmltree::Node;

pub fn extract_documentation(node: Node) -> Option<String> {
    node.children()
        .find(|n| n.tag_name().name() == "annotation")
        .and_then(|n| {
            n.children()
                .find(|c| c.tag_name().name() == "documentation")
        })
        .and_then(|n| n.text().map(|t| t.trim().to_string()))
}

pub fn get_child_elements(node: Node) -> Vec<ChildInfo> {
    let mut children = Vec::new();

    // Helper recursive function to find elements
    fn find_elements_recursive<'a>(n: Node<'a, 'a>, list: &mut Vec<ChildInfo>) {
        for child in n.children() {
            if child.tag_name().name() == "element" {
                let name = child
                    .attribute("ref")
                    .or_else(|| child.attribute("name"))
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    let min_occurs = child.attribute("minOccurs").unwrap_or("1").to_string();
                    let max_occurs = child.attribute("maxOccurs").unwrap_or("1").to_string();
                    let doc = extract_documentation(child);
                    list.push(ChildInfo {
                        name,
                        min_occurs,
                        max_occurs,
                        doc,
                    });
                }
            } else if matches!(
                child.tag_name().name(),
                "sequence" | "choice" | "all" | "complexType" | "complexContent" | "extension"
            ) {
                find_elements_recursive(child, list);
            }
        }
    }

    find_elements_recursive(node, &mut children);
    children
}

pub fn get_attributes(node: Node) -> Vec<AttributeInfo> {
    let mut attrs = Vec::new();

    // Helper to extract from a node that can contain attributes
    let extract_from = |n: Node, list: &mut Vec<AttributeInfo>| {
        for child in n.children() {
            if child.tag_name().name() == "attribute" {
                let name = child.attribute("name").unwrap_or("").to_string();
                if !name.is_empty() {
                    let type_name = child.attribute("type").unwrap_or("").to_string();
                    let use_attr = child.attribute("use").unwrap_or("optional").to_string();
                    let doc = extract_documentation(child);
                    list.push(AttributeInfo {
                        name,
                        type_name,
                        use_attr,
                        doc,
                    });
                }
            }
        }
    };

    // 1. Check if inline complexType
    if let Some(complex) = node
        .children()
        .find(|n| n.tag_name().name() == "complexType")
    {
        extract_from(complex, &mut attrs);

        // Check content models (simpleContent, complexContent)
        for content in complex.children() {
            if content.tag_name().name() == "simpleContent"
                || content.tag_name().name() == "complexContent"
            {
                if let Some(extension) = content
                    .children()
                    .find(|n| n.tag_name().name() == "extension")
                {
                    extract_from(extension, &mut attrs);
                }
            }
        }
    }

    attrs
}

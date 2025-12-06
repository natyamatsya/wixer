use std::collections::HashMap;

pub fn generate_element_mermaid(
    element_name: &str,
    relationships: &HashMap<String, Vec<String>>,
) -> String {
    let mut lines = Vec::new();
    lines.push("graph TD".to_string());
    lines.push(format!("    {0}[{0}]", element_name));

    if let Some(children) = relationships.get(element_name) {
        for child in children {
            let child_id = child.replace('-', "_");
            lines.push(format!("    {} --> {}[{}]", element_name, child_id, child));

            // Grandchildren
            if let Some(grandchildren) = relationships.get(child) {
                for grandchild in grandchildren.iter().take(3) {
                    // Limit to 3
                    let grandchild_id = grandchild.replace('-', "_");
                    lines.push(format!(
                        "    {} --> {}[{}]",
                        child_id, grandchild_id, grandchild
                    ));
                }
            }
        }
    }
    lines.join("\n")
}

use crate::describe_db::{self, WixProperty};
use crate::embedded_assets;
use anyhow::Result;
use strsim::jaro_winkler;
use tracing::warn;

pub fn run(term: String) -> Result<()> {
    if term.contains('.') {
        match resolve_path_query(&term) {
            Ok(Some(result)) => {
                println!("\n--- {} ({}) [Path] ---", result.name, result.kind);
                if !result.tags.is_empty() {
                    println!("Tags: {}", result.tags.join(", "));
                }
                println!("{}", result.description);
                if let Some(url) = &result.docs_url {
                    println!("Docs: {}", url);
                }
                return Ok(());
            }
            Ok(None) => {
                // fall through
            }
            Err(e) => {
                warn!("Failed to resolve path query '{}': {}", term, e);
            }
        }
    }

    let term_lower = term.to_lowercase();

    // Load DB from embedded assets
    let properties = describe_db::load().unwrap_or_else(|e| {
        warn!("Failed to load embedded description database: {}", e);
        vec![]
    });

    if properties.is_empty() {
        println!("Description database is empty or could not be loaded.");
        return Ok(());
    }

    // Fuzzy Match Strategy
    // 1. Exact contains match (Case Insensitive) -> Score 1.0
    // 2. Jaro-Winkler Similarity -> Score [0.0, 1.0]

    let mut matches: Vec<(&WixProperty, f64)> = properties
        .iter()
        .map(|p| {
            let name_lower = p.name.to_lowercase();
            let score = if name_lower.contains(&term_lower) {
                // Boost exact substring match
                // Prefer shorter names for same substring (e.g. "File" vs "RemoveFile" when searching "File")
                1.0 + (1.0 / (p.name.len() as f64 + 1.0))
            } else {
                jaro_winkler(&name_lower, &term_lower)
            };
            (p, score)
        })
        .filter(|(_, score)| *score > 0.85) // Threshold
        .collect();

    // Sort by score descending
    matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    if matches.is_empty() {
        println!("No matches found for '{}'", term);
        return Ok(());
    }

    // Limit output to top 10 to avoid flooding
    let display_count = matches.len().min(10);
    println!(
        "Found {} matches (showing top {}):",
        matches.len(),
        display_count
    );

    for (p, score) in matches.iter().take(display_count) {
        let confidence = if *score >= 1.0 { "Exact" } else { "Fuzzy" };
        println!("\n--- {} ({}) [{}] ---", p.name, p.kind, confidence);
        if !p.tags.is_empty() {
            println!("Tags: {}", p.tags.join(", "));
        }

        // Wrap description nicely
        let desc = if p.description.len() > 300 {
            format!("{}...", &p.description[0..300])
        } else {
            p.description.clone()
        };
        println!("{}", desc);

        if let Some(url) = &p.docs_url {
            println!("Docs: {}", url);
        }
    }

    Ok(())
}

fn resolve_path_query(term: &str) -> Result<Option<WixProperty>> {
    let parts: Vec<&str> = term.split('.').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Ok(None);
    }

    let schema_names = embedded_assets::list_json_schemas().unwrap_or_default();
    let mut schema_bases = Vec::new();
    for n in schema_names {
        let mut base = n.as_str();
        if let Some(stripped) = base.strip_suffix(".json") {
            base = stripped;
        }
        if let Some(stripped) = base.strip_suffix(".schema") {
            base = stripped;
        }
        schema_bases.push(base.to_string());
    }

    let (schema_opt, rest) = if parts.len() >= 2 && schema_bases.iter().any(|s| s == parts[0]) {
        (Some(parts[0]), &parts[1..])
    } else {
        (None, parts.as_slice())
    };

    let mut matches = Vec::new();
    let schemas_to_try: Vec<&str> = if let Some(schema) = schema_opt {
        vec![schema]
    } else {
        schema_bases.iter().map(|s| s.as_str()).collect()
    };

    for schema in schemas_to_try {
        if let Some(found) = resolve_in_schema(schema, rest)? {
            matches.push(found);
        }
    }

    if matches.is_empty() {
        return Ok(None);
    }

    if matches.len() == 1 {
        return Ok(Some(matches.remove(0)));
    }

    println!("Ambiguous path '{}'. Candidates:", term);
    for m in matches.iter().take(20) {
        println!("- {}", m.name);
    }
    Ok(None)
}

fn resolve_in_schema(schema: &str, parts: &[&str]) -> Result<Option<WixProperty>> {
    if parts.is_empty() {
        return Ok(None);
    }

    let schema_file = format!("{}.schema.json", schema);
    let Ok(content) = embedded_assets::get_json_schema(&schema_file) else {
        return Ok(None);
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Ok(None);
    };

    let Some(definitions) = json.get("definitions").and_then(|d| d.as_object()) else {
        return Ok(None);
    };

    let element = parts[0];
    let Some(def) = definitions.get(element) else {
        return Ok(None);
    };

    let (name, kind, description) = match parts.len() {
        1 => {
            let Some(desc) = def.get("description").and_then(|d| d.as_str()) else {
                return Ok(None);
            };
            (
                format!("{}.{}", schema, element),
                "Element".to_string(),
                desc.to_string(),
            )
        }
        2 => {
            let attr = parts[1];
            let Some(props) = def.get("properties").and_then(|p| p.as_object()) else {
                return Ok(None);
            };
            let Some(attr_schema) = props.get(attr) else {
                return Ok(None);
            };
            let Some(desc) = attr_schema.get("description").and_then(|d| d.as_str()) else {
                return Ok(None);
            };
            (
                format!("{}.{}.{}", schema, element, attr),
                "Attribute".to_string(),
                desc.to_string(),
            )
        }
        _ => {
            return Ok(None);
        }
    };

    Ok(Some(WixProperty {
        name,
        kind,
        description,
        docs_url: None,
        tags: vec![schema.to_string()],
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn path_query_parser_splits_segments() {
        let term = "wix.CustomAction.Id";
        let parts: Vec<&str> = term.split('.').filter(|p| !p.is_empty()).collect();
        assert_eq!(parts, vec!["wix", "CustomAction", "Id"]);
    }
}

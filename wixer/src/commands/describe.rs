use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct WixProperty {
    name: String,
    kind: String, // "Property", "Directory", "Preprocessor", "CPack"
    description: String,
    docs_url: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

lazy_static::lazy_static! {
    static ref PROPERTIES: Vec<WixProperty> = {
        let json = include_str!("../../assets/knowledge_base.json");
        serde_json::from_str(json).expect("Failed to parse knowledge_base.json")
    };
}

pub fn run(term: String) -> Result<()> {
    let term_lower = term.to_lowercase();

    let matches: Vec<&WixProperty> = PROPERTIES
        .iter()
        .filter(|p| p.name.to_lowercase().contains(&term_lower))
        .collect();

    if matches.is_empty() {
        println!("No matches found for '{}'", term);
        return Ok(());
    }

    println!("Found {} matches:", matches.len());
    for p in matches {
        println!("\n--- {} ({}) ---", p.name, p.kind);
        if !p.tags.is_empty() {
            println!("Tags: {}", p.tags.join(", "));
        }
        println!("{}", p.description);
        if let Some(url) = &p.docs_url {
            println!("Docs: {}", url);
        }
    }

    Ok(())
}

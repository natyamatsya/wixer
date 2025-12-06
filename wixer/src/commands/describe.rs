use crate::describe_db::{self, WixProperty};
use crate::utils::find_wixer_root;
use anyhow::Result;
use tracing::warn;

pub fn run(term: String) -> Result<()> {
    let term_lower = term.to_lowercase();

    // Find project root to load DB
    let properties = if let Ok(root) = find_wixer_root() {
        describe_db::load(&root).unwrap_or_else(|e| {
            warn!(
                "Failed to load description database: {}. Run 'wixer init' to generate it.",
                e
            );
            vec![]
        })
    } else {
        warn!("Not in a wixer project. Description database not loaded. Run 'wixer init'.");
        vec![]
    };

    if properties.is_empty() {
        println!("Description database is empty or could not be loaded.");
        println!("Try running 'wixer init' to generate it.");
        return Ok(());
    }

    let matches: Vec<&WixProperty> = properties
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

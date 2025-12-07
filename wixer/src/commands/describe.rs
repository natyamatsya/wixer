use crate::describe_db::{self, WixProperty};
use anyhow::Result;
use tracing::warn;
use strsim::jaro_winkler;

pub fn run(term: String) -> Result<()> {
    let term_lower = term.to_lowercase();

    // Load DB from global cache
    let properties = describe_db::load().unwrap_or_else(|e| {
        warn!(
            "Failed to load description database: {}. Run 'wixer init' to generate it.",
            e
        );
        vec![]
    });

    if properties.is_empty() {
        println!("Description database is empty or could not be loaded.");
        println!("Try running 'wixer init' to generate it.");
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
    println!("Found {} matches (showing top {}):", matches.len(), display_count);
    
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

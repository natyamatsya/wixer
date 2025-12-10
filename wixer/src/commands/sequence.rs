use crate::analysis::wix_action::parse_wix_action;
use crate::wixpdb::WixPdb;
use anyhow::Result;
use std::path::PathBuf;
use tabled::{Table, Tabled, settings::Style};
use tracing::info;

#[derive(Tabled)]
struct SequenceRow {
    #[tabled(rename = "Table")]
    table: String,
    #[tabled(rename = "Seq")]
    sequence: String,
    #[tabled(rename = "Action")]
    action: String,
    #[tabled(rename = "Condition")]
    condition: String,
}

pub fn run(path: &PathBuf, table_filter: Option<String>) -> Result<()> {
    info!("Loading PDB: {}", path.display());
    let pdb = WixPdb::load(path)?;

    let mut actions = Vec::new();

    for section in &pdb.sections {
        for symbol in &section.symbols {
            if symbol.symbol_type == "WixAction" {
                // Determine ID (needed for table name)
                let id_str = symbol
                    .id
                    .get("id")
                    .and_then(|v| v.as_str())
                    .or_else(|| symbol.id.as_str())
                    .unwrap_or("?");

                let data = parse_wix_action(id_str, &symbol.fields);
                actions.push(data);
            }
        }
    }

    if let Some(filter) = &table_filter {
        actions.retain(|a| a.table.contains(filter));
    }

    // Sort: Table then Sequence
    actions.sort_by(|a, b| {
        let table_cmp = a.table.cmp(&b.table);
        if table_cmp != std::cmp::Ordering::Equal {
            return table_cmp;
        }
        // Handle missing sequences (put at end)
        let seq_a = a.sequence.unwrap_or(i64::MAX);
        let seq_b = b.sequence.unwrap_or(i64::MAX);
        seq_a.cmp(&seq_b)
    });

    let rows: Vec<SequenceRow> = actions
        .into_iter()
        .map(|a| SequenceRow {
            table: a.table,
            sequence: a
                .sequence
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string()),
            action: a.action,
            condition: a.condition.unwrap_or_default(),
        })
        .collect();

    if rows.is_empty() {
        println!("No actions found.");
    } else {
        let table = Table::new(rows).with(Style::modern()).to_string();
        println!("{}", table);
    }

    Ok(())
}

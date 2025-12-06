use crate::wixpdb::WixPdb;
use crate::analysis::{custom_action, util_extension};
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;
use tabled::{Table, Tabled, settings::Style};

#[derive(Tabled)]
struct SymbolRow {
    #[tabled(rename = "Type")]
    symbol_type: String,
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Source File")]
    file: String,
    #[tabled(rename = "Line")]
    line: String,
}

pub fn run(path: &PathBuf, symbol: Option<String>) -> Result<()> {
    info!("Loading PDB: {}", path.display());
    let pdb = WixPdb::load(path)?;

    if let Some(id) = symbol {
        info!("Searching for symbol: {}", id);
        let symbols = pdb.find_symbol(&id);
        if symbols.is_empty() {
            info!("Symbol not found.");
        } else {
            for s in symbols {
                println!("\nSymbol Details:");
                println!("  ID: {}", id);
                println!("  Type: {}", s.symbol_type);
                if let Some(ln) = &s.ln {
                    let line_str = ln.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string());
                    println!("  Location: {}:{}", ln.file, line_str);
                } else {
                    println!("  Location: <unknown>");
                }
                
                // --- Semantic Analysis ---
                match s.symbol_type.as_str() {
                    "CustomAction" => {
                        println!("\n[Analyzer Report]");
                        println!("{}", custom_action::analyze_launch_app(&s.fields));
                    },
                    "WixCloseApplication" => {
                        println!("\n[Analyzer Report]");
                        println!("{}", util_extension::analyze_close_app(&s.fields));
                    },
                    _ => {}
                }
                // -------------------------

                if !s.fields.is_empty() {
                    println!("\nFields:");
                    for (i, field) in s.fields.iter().enumerate() {
                        println!("  [{}] {:?}", i, field);
                    }
                }
                println!("----------------------------------------");
            }
        }
    } else {
        let mut rows = Vec::new();
        for section in pdb.sections {
            for s in section.symbols {
                 // Try to get ID string
                 let id_str = s.id.get("id").and_then(|v| v.as_str())
                    .or_else(|| s.id.as_str())
                    .unwrap_or("?").to_string();
                 
                 let (file, line) = if let Some(ln) = &s.ln {
                     (ln.file.clone(), ln.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string()))
                 } else {
                     ("-".to_string(), "-".to_string())
                 };

                 rows.push(SymbolRow {
                     symbol_type: s.symbol_type,
                     id: id_str,
                     file,
                     line,
                 });
            }
        }

        // Sort by Type then ID
        rows.sort_by(|a, b| {
            a.symbol_type.cmp(&b.symbol_type).then(a.id.cmp(&b.id))
        });

        if !rows.is_empty() {
            let table = Table::new(rows).with(Style::modern()).to_string();
            println!("{}", table);
        } else {
            info!("No symbols found in PDB.");
        }
    }

    Ok(())
}

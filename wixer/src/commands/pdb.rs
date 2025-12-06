use crate::wixpdb::WixPdb;
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;

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
                if let Some(ln) = &s.ln {
                    let line_str = ln.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string());
                    info!("Found symbol '{}' at {}:{}", id, ln.file, line_str);
                } else {
                    info!("Found symbol '{}' (no source location)", id);
                }
            }
        }
    } else {
        info!("Listing all symbols:");
        for section in pdb.sections {
            for s in section.symbols {
                 // Try to get ID string
                 let id_str = s.id.get("id").and_then(|v| v.as_str())
                    .or_else(|| s.id.as_str())
                    .unwrap_or("?");
                 if let Some(ln) = &s.ln {
                     let line_str = ln.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string());
                     println!("{} ({}) -> {}:{}", id_str, s.symbol_type, ln.file, line_str);
                 } else {
                     println!("{} ({})", id_str, s.symbol_type);
                 }
            }
        }
    }

    Ok(())
}

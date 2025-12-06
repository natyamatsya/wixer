use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
pub struct WixPdb {
    #[serde(default)]
    pub sections: Vec<Section>,
}

#[derive(Debug, Deserialize)]
pub struct Section {
    #[serde(default)]
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Deserialize)]
pub struct Symbol {
    #[serde(rename = "type")]
    pub symbol_type: String,
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub ln: Option<SourceLocation>,
    #[serde(default)]
    pub fields: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    #[serde(default)]
    pub line: Option<u64>,
}

impl WixPdb {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path).context("Failed to open PDB file")?;
        let mut archive = ZipArchive::new(file).context("Failed to open ZIP archive")?;

        let mut json_file = archive
            .by_name("wix-ir.json")
            .context("wix-ir.json not found in PDB")?;

        let mut content = String::new();
        json_file
            .read_to_string(&mut content)
            .context("Failed to read wix-ir.json")?;

        let pdb: WixPdb = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                println!("JSON Parse Error: {}", e);
                return Err(anyhow::anyhow!("Failed to parse wix-ir.json: {}", e));
            }
        };
        Ok(pdb)
    }

    pub fn find_symbol(&self, id: &str) -> Vec<&Symbol> {
        let mut results = Vec::new();
        for section in &self.sections {
            for symbol in &section.symbols {
                if let Some(symbol_id) = symbol.id.get("id") {
                    if symbol_id.as_str() == Some(id) {
                        results.push(symbol);
                    }
                } else if let Some(symbol_id_str) = symbol.id.as_str()
                    && symbol_id_str == id {
                        results.push(symbol);
                    }
            }
        }
        results
    }
}

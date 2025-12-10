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

        // WiX's JSON serializer doesn't properly escape control characters in string fields
        // (e.g., RTF content with literal \r\n). We need to sanitize the JSON before parsing.
        let content = Self::sanitize_json_control_chars(&content);

        let pdb: WixPdb = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                println!("JSON Parse Error: {}", e);
                return Err(anyhow::anyhow!("Failed to parse wix-ir.json: {}", e));
            }
        };
        Ok(pdb)
    }

    /// Sanitize control characters in JSON strings that WiX doesn't properly escape
    fn sanitize_json_control_chars(json: &str) -> String {
        let mut result = String::with_capacity(json.len());
        let mut in_string = false;
        let mut escape_next = false;

        for ch in json.chars() {
            if escape_next {
                result.push(ch);
                escape_next = false;
                continue;
            }

            if ch == '\\' && in_string {
                result.push(ch);
                escape_next = true;
                continue;
            }

            if ch == '"' {
                in_string = !in_string;
                result.push(ch);
                continue;
            }

            if in_string {
                // Escape control characters that should be escaped in JSON strings
                match ch {
                    '\r' => result.push_str("\\r"),
                    '\n' => result.push_str("\\n"),
                    '\t' => result.push_str("\\t"),
                    c if c.is_control() => {
                        // Escape other control characters as \uXXXX
                        result.push_str(&format!("\\u{:04x}", c as u32));
                    }
                    _ => result.push(ch),
                }
            } else {
                result.push(ch);
            }
        }

        result
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
                    && symbol_id_str == id
                {
                    results.push(symbol);
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_json_control_chars() {
        // Test that control characters inside strings are escaped
        let input = r#"{"data":"line1\r\nline2"}"#;
        // Already escaped - should pass through
        let result = WixPdb::sanitize_json_control_chars(input);
        assert_eq!(result, input);

        // Test with actual control characters (simulating WiX's broken output)
        let input_with_controls = "{\"data\":\"line1\r\nline2\"}";
        let result = WixPdb::sanitize_json_control_chars(input_with_controls);
        assert_eq!(result, r#"{"data":"line1\r\nline2"}"#);

        // Test that control chars outside strings are preserved
        let input_outside = "{\n\"key\": \"value\"\n}";
        let result = WixPdb::sanitize_json_control_chars(input_outside);
        assert_eq!(result, input_outside);
    }
}

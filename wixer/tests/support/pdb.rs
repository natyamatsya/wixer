use serde_json::json;

use wixer::wixpdb::{Section, Symbol, WixPdb};

pub fn custom_action_symbol(id: &str, fields: Vec<serde_json::Value>) -> Symbol {
    Symbol {
        symbol_type: "CustomAction".to_string(),
        id: json!({"id": id}),
        ln: None,
        fields,
    }
}

pub fn wix_action_symbol(id: serde_json::Value, fields: Vec<serde_json::Value>) -> Symbol {
    Symbol {
        symbol_type: "WixAction".to_string(),
        id,
        ln: None,
        fields,
    }
}

pub fn pdb_with_symbols(symbols: Vec<Symbol>) -> WixPdb {
    WixPdb {
        sections: vec![Section { symbols }],
    }
}

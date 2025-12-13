pub struct PdbBuilder {
    symbols: Vec<wixer::wixpdb::Symbol>,
}

impl PdbBuilder {
    pub fn new() -> Self {
        Self { symbols: vec![] }
    }

    pub fn custom_action(self, id: impl Into<String>) -> PdbSymbolBuilder {
        let id = id.into();
        let symbol = wixer::wixpdb::Symbol {
            symbol_type: "CustomAction".to_string(),
            id: serde_json::json!({"id": id}),
            ln: None,
            fields: vec![],
        };
        PdbSymbolBuilder { pdb: self, symbol }
    }

    pub fn build(self) -> wixer::wixpdb::WixPdb {
        wixer::wixpdb::WixPdb {
            sections: vec![wixer::wixpdb::Section {
                symbols: self.symbols,
            }],
        }
    }
}

pub struct PdbSymbolBuilder {
    pdb: PdbBuilder,
    symbol: wixer::wixpdb::Symbol,
}

impl PdbSymbolBuilder {
    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.symbol
            .fields
            .push(serde_json::json!({"Condition": condition.into()}));
        self
    }

    pub fn after_action(mut self, after: impl Into<String>) -> Self {
        self.symbol
            .fields
            .push(serde_json::json!({"After": after.into()}));
        self
    }

    pub fn finish(mut self) -> PdbBuilder {
        self.pdb.symbols.push(self.symbol);
        self.pdb
    }
}

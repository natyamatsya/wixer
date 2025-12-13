use std::sync::OnceLock;

pub fn yaml(s: &str) -> serde_yaml::Value {
    serde_yaml::from_str(s).unwrap()
}

pub mod registry {
    use super::*;

    use wixer::simulation::{ExtensionRegistry, ExtractedExtension};

    static BUNDLED: OnceLock<ExtensionRegistry> = OnceLock::new();

    pub fn bundled() -> &'static ExtensionRegistry {
        BUNDLED.get_or_init(ExtensionRegistry::load_bundled)
    }

    pub fn ui() -> &'static ExtractedExtension {
        bundled().get("ui").expect("ui extension must be bundled")
    }
}

pub mod pdb {
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
}

pub mod extension {
    use wixer::simulation::{ExtractedExtension, ExtractedLicense};

    pub fn dummy_extension(id: &str) -> ExtractedExtension {
        ExtractedExtension {
            license: ExtractedLicense {
                notice: "n".into(),
                source: "s".into(),
                spdx_id: "MIT".into(),
            },
            id: id.into(),
            namespace: None,
            custom_actions: vec![],
            sequence_entries: vec![],
            publish_entries: vec![],
            properties: vec![],
            binaries: vec![],
        }
    }
}

pub struct SimBuilder {
    sim: wixer::simulation::Simulator,
}

impl SimBuilder {
    pub fn new() -> Self {
        Self {
            sim: wixer::simulation::Simulator::new(),
        }
    }

    pub fn custom_action(mut self, action: wixer::simulation::Action) -> Self {
        self.sim.add_custom_action(action);
        self
    }

    pub fn action(self, builder: ActionBuilder) -> Self {
        self.custom_action(builder.build())
    }

    pub fn build(self) -> wixer::simulation::Simulator {
        self.sim
    }
}

impl Default for SimBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ActionBuilder {
    action: wixer::simulation::Action,
}

impl ActionBuilder {
    pub fn new_custom(id: impl Into<String>, ca_type: wixer::simulation::CustomActionType) -> Self {
        Self {
            action: wixer::simulation::Action::new_custom(id, ca_type),
        }
    }

    pub fn dll_binary(
        id: impl Into<String>,
        dll_entry: impl Into<String>,
        binary_ref: impl Into<String>,
    ) -> Self {
        Self::new_custom(
            id,
            wixer::simulation::CustomActionType::DllBinary {
                dll_entry: dll_entry.into(),
                binary_ref: binary_ref.into(),
            },
        )
    }

    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.action.condition = Some(condition.into());
        self
    }

    pub fn before(mut self, before_action: impl Into<String>) -> Self {
        self.action.position = Some(wixer::simulation::SequencePosition::Before(
            before_action.into(),
        ));
        self
    }

    pub fn after(mut self, after_action: impl Into<String>) -> Self {
        self.action.position = Some(wixer::simulation::SequencePosition::After(
            after_action.into(),
        ));
        self
    }

    pub fn build(self) -> wixer::simulation::Action {
        self.action
    }
}

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

    pub fn wix_action(self, id: serde_json::Value) -> PdbSymbolBuilder {
        let symbol = wixer::wixpdb::Symbol {
            symbol_type: "WixAction".to_string(),
            id,
            ln: None,
            fields: vec![],
        };
        PdbSymbolBuilder { pdb: self, symbol }
    }

    pub fn symbol(mut self, symbol: wixer::wixpdb::Symbol) -> Self {
        self.symbols.push(symbol);
        self
    }

    pub fn build(self) -> wixer::wixpdb::WixPdb {
        wixer::wixpdb::WixPdb {
            sections: vec![wixer::wixpdb::Section {
                symbols: self.symbols,
            }],
        }
    }
}

impl Default for PdbBuilder {
    fn default() -> Self {
        Self::new()
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

    pub fn before_action(mut self, before: impl Into<String>) -> Self {
        self.symbol
            .fields
            .push(serde_json::json!({"Before": before.into()}));
        self
    }

    pub fn sequence_table(mut self, table: impl Into<String>) -> Self {
        self.symbol
            .fields
            .push(serde_json::json!({"SequenceTable": table.into()}));
        self
    }

    pub fn sequence(mut self, seq: i32) -> Self {
        self.symbol
            .fields
            .push(serde_json::json!({"Sequence": seq}));
        self
    }

    pub fn field(mut self, value: serde_json::Value) -> Self {
        self.symbol.fields.push(value);
        self
    }

    pub fn finish(mut self) -> PdbBuilder {
        self.pdb.symbols.push(self.symbol);
        self.pdb
    }
}

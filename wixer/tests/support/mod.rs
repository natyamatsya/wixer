use serde_yaml::Value;

pub fn yaml_get<'a>(val: &'a Value, key: &str) -> &'a Value {
    match val {
        Value::Mapping(map) => map
            .get(Value::String(key.to_string()))
            .unwrap_or_else(|| panic!("missing key: {}", key)),
        _ => panic!("expected mapping"),
    }
}

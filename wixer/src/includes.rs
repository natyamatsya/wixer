use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde_yaml::{Mapping, Value};
use std::borrow::Cow;
use std::fs;
use std::path::Path;
use tracing::error;

// Marker prefix for internal handling
const INCLUDE_MARKER: &str = "__wixer_include__";
const MAX_RECURSION_DEPTH: usize = 100;

pub fn has_includes(content: &str) -> bool {
    content.contains("!include")
}

/// deeply merges `update` into `base`.
/// - Maps are merged recursively.
/// - Sequences are appended.
/// - Scalars overwrite.
pub fn deep_merge(base: &mut Value, update: Value) {
    match (base, update) {
        (Value::Mapping(base_map), Value::Mapping(update_map)) => {
            for (key, value) in update_map {
                if let Some(base_value) = base_map.get_mut(&key) {
                    deep_merge(base_value, value);
                } else {
                    base_map.insert(key, value);
                }
            }
        }
        (Value::Sequence(base_seq), Value::Sequence(update_seq)) => {
            base_seq.extend(update_seq);
        }
        (base_val, update_val) => {
            *base_val = update_val;
        }
    }
}

/// Pre-processes the YAML text to convert `!include` directives into string tokens
/// that can be parsed by serde_yaml.
///
/// Distinguishes between:
/// - Mixin context (Key): `!include foo.yaml` -> `"__wixer_include__ foo.yaml": null`
/// - Value context: `Key: !include foo.yaml` -> `Key: { "__wixer_include__ foo.yaml": null }`
fn preprocess_text<'a>(text: &'a str) -> Cow<'a, str> {
    if !has_includes(text) {
        return Cow::Borrowed(text);
    }

    let re = Regex::new(r#"!include\s+(".*?"|\S+)"#).unwrap();

    let mut result = String::with_capacity(text.len());
    let mut last_match_end = 0;

    for cap in re.captures_iter(text) {
        let mat = cap.get(0).unwrap();
        let path = cap.get(1).unwrap().as_str();

        // Append text before match
        result.push_str(&text[last_match_end..mat.start()]);

        // Determine context by looking backwards
        let prefix = &text[..mat.start()];
        let mut is_same_line = false;

        // Scan backwards for last non-whitespace char
        for c in prefix.chars().rev() {
            if c == '\n' {
                is_same_line = false;
                break;
            }
            if !c.is_whitespace() {
                is_same_line = true;
                break;
            }
        }

        // If path is quoted, keep quotes, else quote it
        let quoted_path = if path.starts_with('"') || path.starts_with('\'') {
            path.to_string()
        } else {
            format!("\"{}\"", path)
        };

        let key_str = format!(
            "\"{} {}\"",
            INCLUDE_MARKER,
            quoted_path.trim_matches(|c| c == '"' || c == '\'')
        );

        if is_same_line {
            // Value context: Use Flow Map syntax
            result.push_str(&format!("{{ {}: null }}", key_str));
        } else {
            // Key context (Mixin): Use Block Key syntax
            result.push_str(&format!("{}: null", key_str));
        }

        last_match_end = mat.end();
    }

    result.push_str(&text[last_match_end..]);
    Cow::Owned(result)
}

pub fn load_and_process_yaml(path: &Path) -> Result<Value> {
    load_and_process_yaml_with_depth(path, 0)
}

fn load_and_process_yaml_with_depth(path: &Path, depth: usize) -> Result<Value> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(anyhow!(
            "Maximum recursion depth ({}) exceeded while loading {}",
            MAX_RECURSION_DEPTH,
            path.display()
        ));
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    process_yaml_str_with_depth(&content, path, depth)
}

#[allow(dead_code)]
fn process_yaml_str(content: &str, base_path: &Path) -> Result<Value> {
    process_yaml_str_with_depth(content, base_path, 0)
}

fn process_yaml_str_with_depth(content: &str, base_path: &Path, depth: usize) -> Result<Value> {
    let preprocessed = preprocess_text(content);
    let value: Value = match serde_yaml::from_str(&preprocessed) {
        Ok(v) => v,
        Err(e) => {
            // Log the preprocessed content for debugging
            error!(
                "Failed to parse preprocessed YAML for {}:\n{}",
                base_path.display(),
                preprocessed
            );
            return Err(anyhow::anyhow!(
                "Failed to parse YAML in {}: {}",
                base_path.display(),
                e
            ));
        }
    };
    resolve_includes_recursive(value, base_path, depth)
}

fn resolve_includes_recursive(val: Value, base_path: &Path, depth: usize) -> Result<Value> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(anyhow!(
            "Maximum recursion depth ({}) exceeded while resolving includes in {}",
            MAX_RECURSION_DEPTH,
            base_path.display()
        ));
    }

    match val {
        Value::Mapping(map) => resolve_mapping(map, base_path, depth),
        Value::Sequence(seq) => resolve_sequence(seq, base_path, depth),
        _ => Ok(val),
    }
}

fn resolve_mapping(map: Mapping, base_path: &Path, depth: usize) -> Result<Value> {
    // Check if this map is purely a "Value Include" wrapper
    // i.e. size 1, key is marker
    if map.len() == 1
        && let Some((Value::String(s), _)) = map.iter().next()
        && s.starts_with(INCLUDE_MARKER)
    {
        let path_str = s.strip_prefix(INCLUDE_MARKER).unwrap().trim();
        let include_path = base_path.parent().unwrap_or(Path::new(".")).join(path_str);
        return load_and_process_yaml_with_depth(&include_path, depth + 1);
    }

    // Normal Map with possible mixins
    let mut result_map = Mapping::new();
    let mut mixins = Vec::new();
    let mut regular = Vec::new();

    for (k, v) in map {
        if let Value::String(s) = &k
            && s.starts_with(INCLUDE_MARKER)
        {
            let path_str = s.strip_prefix(INCLUDE_MARKER).unwrap().trim();
            mixins.push(path_str.to_string());
            continue;
        }
        regular.push((k, v));
    }

    // Apply mixins first
    for include_path_str in mixins {
        let include_path = base_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(include_path_str);
        let included_val = load_and_process_yaml_with_depth(&include_path, depth + 1)?;

        match included_val {
            Value::Mapping(included_map) => {
                let mut temp_base = Value::Mapping(result_map);
                deep_merge(&mut temp_base, Value::Mapping(included_map));
                if let Value::Mapping(m) = temp_base {
                    result_map = m;
                } else {
                    panic!("Merge corrupted structure");
                }
            }
            _ => {
                return Err(anyhow!(
                    "Included file {} is not a mapping, cannot be mixed into a mapping",
                    include_path.display()
                ));
            }
        }
    }

    // Apply regular keys (overrides)
    for (k, v) in regular {
        let resolved_v = resolve_includes_recursive(v, base_path, depth + 1)?;
        if let Some(existing_v) = result_map.get_mut(&k) {
            deep_merge(existing_v, resolved_v);
        } else {
            result_map.insert(k, resolved_v);
        }
    }

    Ok(Value::Mapping(result_map))
}

fn resolve_sequence(seq: Vec<Value>, base_path: &Path, depth: usize) -> Result<Value> {
    let mut result_seq = Vec::new();

    for item in seq {
        // Check if item is an include wrapper
        let mut is_include = false;
        let mut include_path_str = String::new();

        if let Value::Mapping(ref m) = item
            && m.len() == 1
            && let Some((Value::String(s), _)) = m.iter().next()
            && s.starts_with(INCLUDE_MARKER)
        {
            is_include = true;
            include_path_str = s.strip_prefix(INCLUDE_MARKER).unwrap().trim().to_string();
        }

        if is_include {
            let include_path = base_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(include_path_str);
            let included_val = load_and_process_yaml_with_depth(&include_path, depth + 1)?;

            if let Value::Sequence(inner_seq) = included_val {
                result_seq.extend(inner_seq);
            } else {
                result_seq.push(included_val);
            }
        } else {
            result_seq.push(resolve_includes_recursive(item, base_path, depth + 1)?);
        }
    }

    Ok(Value::Sequence(result_seq))
}

#[cfg(test)]
mod tests {
    // tests live under wixer/tests/includes_semantics.rs
}

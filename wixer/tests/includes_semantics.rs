use std::fs;
use std::path::PathBuf;

use wixer::includes;

fn make_temp_dir(prefix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "wixer_test_{}_{}_{}",
        prefix,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn includes_load_and_process_yaml_supports_value_and_mixin_context_and_quoted_paths() {
    let dir = make_temp_dir("includes_semantics");

    let mixin_dir = dir.join("dir with spaces");
    fs::create_dir_all(&mixin_dir).unwrap();

    fs::write(mixin_dir.join("mixin.yaml"), "A: 1\n").unwrap();
    fs::write(mixin_dir.join("value.yaml"), "B: 2\n").unwrap();

    let main = dir.join("main.yaml");
    fs::write(
        &main,
        r#"Root:
  !include "dir with spaces/mixin.yaml"
  Child: !include "dir with spaces/value.yaml"
"#,
    )
    .unwrap();

    let v = includes::load_and_process_yaml(&main).unwrap();

    let root = v.get("Root").unwrap();
    assert_eq!(root.get("A").unwrap().as_i64().unwrap(), 1);

    let child = root.get("Child").unwrap();
    assert_eq!(child.get("B").unwrap().as_i64().unwrap(), 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn includes_deep_merge_merges_maps_and_appends_sequences() {
    let mut base: serde_yaml::Value = serde_yaml::from_str(
        r#"Key: Base
Nested:
  A: 1
  B: 2
List: [1, 2]
"#,
    )
    .unwrap();

    let update: serde_yaml::Value = serde_yaml::from_str(
        r#"Key: Override
Nested:
  B: 3
  C: 4
List: [3]
"#,
    )
    .unwrap();

    includes::deep_merge(&mut base, update);

    assert_eq!(base.get("Key").unwrap().as_str().unwrap(), "Override");
    let nested = base.get("Nested").unwrap();
    assert_eq!(nested.get("A").unwrap().as_i64().unwrap(), 1);
    assert_eq!(nested.get("B").unwrap().as_i64().unwrap(), 3);
    assert_eq!(nested.get("C").unwrap().as_i64().unwrap(), 4);

    let list = base.get("List").unwrap().as_sequence().unwrap();
    assert_eq!(list.len(), 3);
}

#[test]
fn includes_detects_recursion_limit_for_include_cycles() {
    let dir = make_temp_dir("includes_cycle");

    let a = dir.join("a.yaml");
    let b = dir.join("b.yaml");

    fs::write(&a, "!include b.yaml\n").unwrap();
    fs::write(&b, "!include a.yaml\n").unwrap();

    let err = includes::load_and_process_yaml(&a).unwrap_err();
    assert!(err.to_string().contains("Maximum recursion depth"));

    let _ = fs::remove_dir_all(&dir);
}

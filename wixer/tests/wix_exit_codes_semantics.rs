use regex::Regex;

use wixer::wix_exit_codes;

#[test]
fn wix_exit_codes_mapping_matches_source_table() {
    let src = include_str!("../src/wix_exit_codes.rs");

    let re = Regex::new(r#"(?m)^\s*(\d+)\s*=>\s*Some\(\"([^\"]+)\"\)"#).unwrap();

    let mut pairs = Vec::new();
    for cap in re.captures_iter(src) {
        let code: i32 = cap[1].parse().unwrap();
        let name = cap[2].to_string();
        pairs.push((code, name));
    }

    assert!(!pairs.is_empty());

    for (code, expected) in pairs {
        assert_eq!(
            wix_exit_codes::get_error_name(code),
            Some(expected.as_str())
        );
    }

    assert!(wix_exit_codes::get_error_name(0).is_none());
    assert!(wix_exit_codes::get_error_name(-1).is_none());
    assert!(wix_exit_codes::get_error_name(9_999_999).is_none());
}

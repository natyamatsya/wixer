use std::collections::HashMap;

use wixer::simulation::{ConditionEvaluator, EvalResult, PropertyValue};

fn props_str(pairs: &[(&str, &str)]) -> HashMap<String, PropertyValue> {
    pairs
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                if v.is_empty() {
                    PropertyValue::Null
                } else {
                    PropertyValue::String(v.to_string())
                },
            )
        })
        .collect()
}

#[test]
fn condition_evaluator_handles_literal_truthiness() {
    let p = HashMap::new();

    assert!(ConditionEvaluator::evaluate("1", &p).is_true());
    assert!(!ConditionEvaluator::evaluate("0", &p).is_true());
    assert!(!ConditionEvaluator::evaluate("\"\"", &p).is_true());
    assert!(ConditionEvaluator::evaluate("\"x\"", &p).is_true());
}

#[test]
fn condition_evaluator_handles_parentheses_and_missing_rparen() {
    let p = props_str(&[("Installed", "1")]);
    assert!(ConditionEvaluator::evaluate("(Installed)", &p).is_true());
    assert!(ConditionEvaluator::evaluate("(Installed", &p).is_true());

    let p = props_str(&[]);
    assert!(!ConditionEvaluator::evaluate("(Installed", &p).is_true());
}

#[test]
fn condition_evaluator_supports_property_to_property_comparisons() {
    let p = props_str(&[("A", "X"), ("B", "X"), ("C", "Y")]);

    assert!(ConditionEvaluator::evaluate("A = B", &p).is_true());
    assert!(!ConditionEvaluator::evaluate("A = C", &p).is_true());
}

#[test]
fn condition_evaluator_supports_numeric_comparisons_and_like_operators() {
    let mut p = HashMap::new();
    p.insert("N".to_string(), PropertyValue::Integer(7));
    p.insert("S".to_string(), PropertyValue::String("abcdef".into()));

    assert!(ConditionEvaluator::evaluate("N >= 3", &p).is_true());
    assert!(!ConditionEvaluator::evaluate("N < 3", &p).is_true());

    assert!(ConditionEvaluator::evaluate("S >< \"bcd\"", &p).is_true());
    assert!(ConditionEvaluator::evaluate("S << \"abc\"", &p).is_true());
    assert!(ConditionEvaluator::evaluate("S >> \"def\"", &p).is_true());
}

#[test]
fn condition_evaluator_returns_error_on_missing_comparison_value() {
    let p = props_str(&[("A", "1")]);
    assert!(matches!(
        ConditionEvaluator::evaluate("A =", &p),
        EvalResult::Error(_)
    ));
}

#[test]
fn condition_evaluator_supports_numeric_comparisons_with_string_operands() {
    let p = props_str(&[("S", "7"), ("N", "7")]);

    assert!(ConditionEvaluator::evaluate("S >= 3", &p).is_true());
    assert!(ConditionEvaluator::evaluate("N >= \"3\"", &p).is_true());
}

#[test]
fn condition_evaluator_tokenizes_negative_integers() {
    let p = HashMap::new();
    assert!(ConditionEvaluator::evaluate("-5", &p).is_true());
}

#[test]
fn condition_evaluator_handles_unknown_single_character_tokens_as_properties() {
    let p = HashMap::new();
    assert!(!ConditionEvaluator::evaluate("?", &p).is_true());
}

#[test]
fn condition_evaluator_supports_string_relational_comparisons() {
    let p = props_str(&[("A", "abc"), ("B", "bcd")]);
    assert!(ConditionEvaluator::evaluate("A < B", &p).is_true());
    assert!(ConditionEvaluator::evaluate("B > A", &p).is_true());
}

#[test]
fn condition_evaluator_integer_like_operator_returns_false() {
    let mut p = HashMap::new();
    p.insert("N".to_string(), PropertyValue::Integer(7));
    assert!(!ConditionEvaluator::evaluate("N >< 3", &p).is_true());
}

#[test]
fn condition_evaluator_no_operator_integer_truthiness_works_for_zero() {
    let mut p = HashMap::new();
    p.insert("N".to_string(), PropertyValue::Integer(0));
    assert!(!ConditionEvaluator::evaluate("N", &p).is_true());
}

#[test]
fn condition_evaluator_unexpected_token_defaults_to_false() {
    let p = HashMap::new();
    assert!(!ConditionEvaluator::evaluate(")", &p).is_true());
}

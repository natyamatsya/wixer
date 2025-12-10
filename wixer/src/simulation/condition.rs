//! MSI Condition expression parser and evaluator
//!
//! Supports the Windows Installer condition syntax:
//! - Property comparisons: `PROPERTY`, `PROPERTY = "value"`, `PROPERTY <> "value"`
//! - Numeric comparisons: `PROPERTY >= 3`, `PROPERTY < 100`
//! - Logical operators: `AND`, `OR`, `NOT`
//! - Grouping: `(condition)`
//! - Special: `Installed`, `REMOVE="ALL"`, etc.

use super::model::PropertyValue;
use std::collections::HashMap;

/// Condition evaluation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalResult {
    True,
    False,
    Error(String),
}

impl EvalResult {
    pub fn is_true(&self) -> bool {
        matches!(self, EvalResult::True)
    }
}

/// Parsed condition token
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Property(String),
    StringLiteral(String),
    Integer(i64),
    Operator(Operator),
    LParen,
    RParen,
    And,
    Or,
    Not,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
enum Operator {
    Eq,        // =
    Ne,        // <>
    Lt,        // <
    Le,        // <=
    Gt,        // >
    Ge,        // >=
    Like,      // ><  (substring)
    LeftLike,  // <<  (prefix)
    RightLike, // >>  (suffix)
}

/// Tokenizer for condition expressions
struct Tokenizer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self) -> String {
        let mut s = String::new();
        self.next_char(); // consume opening quote
        while let Some(c) = self.next_char() {
            if c == '"' {
                break;
            }
            s.push(c);
        }
        s
    }

    fn read_identifier(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }
        s
    }

    fn read_number(&mut self) -> i64 {
        let mut s = String::new();
        if self.peek_char() == Some('-') {
            s.push('-');
            self.next_char();
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }
        s.parse().unwrap_or(0)
    }

    fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.peek_char() {
            None => Token::Eof,
            Some('"') => Token::StringLiteral(self.read_string()),
            Some('(') => {
                self.next_char();
                Token::LParen
            }
            Some(')') => {
                self.next_char();
                Token::RParen
            }
            Some('=') => {
                self.next_char();
                Token::Operator(Operator::Eq)
            }
            Some('<') => {
                self.next_char();
                match self.peek_char() {
                    Some('>') => {
                        self.next_char();
                        Token::Operator(Operator::Ne)
                    }
                    Some('=') => {
                        self.next_char();
                        Token::Operator(Operator::Le)
                    }
                    Some('<') => {
                        self.next_char();
                        Token::Operator(Operator::LeftLike)
                    }
                    _ => Token::Operator(Operator::Lt),
                }
            }
            Some('>') => {
                self.next_char();
                match self.peek_char() {
                    Some('=') => {
                        self.next_char();
                        Token::Operator(Operator::Ge)
                    }
                    Some('<') => {
                        self.next_char();
                        Token::Operator(Operator::Like)
                    }
                    Some('>') => {
                        self.next_char();
                        Token::Operator(Operator::RightLike)
                    }
                    _ => Token::Operator(Operator::Gt),
                }
            }
            Some(c) if c.is_ascii_digit() || c == '-' => Token::Integer(self.read_number()),
            Some(c) if c.is_alphabetic() || c == '_' => {
                let ident = self.read_identifier();
                match ident.to_uppercase().as_str() {
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "NOT" => Token::Not,
                    _ => Token::Property(ident),
                }
            }
            Some(c) => {
                self.next_char();
                Token::Property(c.to_string())
            }
        }
    }
}

/// Condition expression evaluator
pub struct ConditionEvaluator {
    tokens: Vec<Token>,
    pos: usize,
}

impl ConditionEvaluator {
    /// Evaluate a condition expression against property state
    pub fn evaluate(condition: &str, properties: &HashMap<String, PropertyValue>) -> EvalResult {
        if condition.trim().is_empty() {
            return EvalResult::True;
        }

        let mut tokenizer = Tokenizer::new(condition);
        let mut tokens = Vec::new();
        loop {
            let tok = tokenizer.next_token();
            if tok == Token::Eof {
                break;
            }
            tokens.push(tok);
        }

        let mut eval = ConditionEvaluator { tokens, pos: 0 };
        eval.parse_or(properties)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn parse_or(&mut self, props: &HashMap<String, PropertyValue>) -> EvalResult {
        let mut left = self.parse_and(props);

        while matches!(self.peek(), Token::Or) {
            self.advance();
            let right = self.parse_and(props);
            left = match (&left, &right) {
                (EvalResult::True, _) | (_, EvalResult::True) => EvalResult::True,
                (EvalResult::Error(e), _) | (_, EvalResult::Error(e)) => {
                    EvalResult::Error(e.clone())
                }
                _ => EvalResult::False,
            };
        }

        left
    }

    fn parse_and(&mut self, props: &HashMap<String, PropertyValue>) -> EvalResult {
        let mut left = self.parse_not(props);

        while matches!(self.peek(), Token::And) {
            self.advance();
            let right = self.parse_not(props);
            left = match (&left, &right) {
                (EvalResult::True, EvalResult::True) => EvalResult::True,
                (EvalResult::Error(e), _) | (_, EvalResult::Error(e)) => {
                    EvalResult::Error(e.clone())
                }
                _ => EvalResult::False,
            };
        }

        left
    }

    fn parse_not(&mut self, props: &HashMap<String, PropertyValue>) -> EvalResult {
        if matches!(self.peek(), Token::Not) {
            self.advance();
            let inner = self.parse_primary(props);
            return match inner {
                EvalResult::True => EvalResult::False,
                EvalResult::False => EvalResult::True,
                e => e,
            };
        }
        self.parse_primary(props)
    }

    fn parse_primary(&mut self, props: &HashMap<String, PropertyValue>) -> EvalResult {
        match self.peek().clone() {
            Token::LParen => {
                self.advance();
                let result = self.parse_or(props);
                if matches!(self.peek(), Token::RParen) {
                    self.advance();
                }
                result
            }
            Token::Property(name) => {
                self.advance();
                self.parse_comparison(&name, props)
            }
            Token::Integer(n) => {
                self.advance();
                if n != 0 {
                    EvalResult::True
                } else {
                    EvalResult::False
                }
            }
            Token::StringLiteral(s) => {
                self.advance();
                if !s.is_empty() {
                    EvalResult::True
                } else {
                    EvalResult::False
                }
            }
            _ => EvalResult::False,
        }
    }

    fn parse_comparison(
        &mut self,
        prop_name: &str,
        props: &HashMap<String, PropertyValue>,
    ) -> EvalResult {
        let prop_value = props.get(prop_name).cloned().unwrap_or(PropertyValue::Null);

        // Check if there's a comparison operator
        if let Token::Operator(op) = self.peek().clone() {
            self.advance();

            let compare_value = match self.advance() {
                Token::StringLiteral(s) => PropertyValue::String(s),
                Token::Integer(n) => PropertyValue::Integer(n),
                Token::Property(p) => props.get(&p).cloned().unwrap_or(PropertyValue::Null),
                _ => return EvalResult::Error("Expected value after operator".into()),
            };

            return self.compare_values(&prop_value, &op, &compare_value);
        }

        // No operator - just check if property is set (non-null, non-empty)
        match &prop_value {
            PropertyValue::Null => EvalResult::False,
            PropertyValue::String(s) => {
                if s.is_empty() {
                    EvalResult::False
                } else {
                    EvalResult::True
                }
            }
            PropertyValue::Integer(n) => {
                if *n != 0 {
                    EvalResult::True
                } else {
                    EvalResult::False
                }
            }
        }
    }

    fn compare_values(
        &self,
        left: &PropertyValue,
        op: &Operator,
        right: &PropertyValue,
    ) -> EvalResult {
        // Handle null comparisons
        let left_str = match left {
            PropertyValue::Null => "",
            PropertyValue::String(s) => s.as_str(),
            PropertyValue::Integer(n) => return self.compare_integers(*n, op, right),
        };

        let right_str = match right {
            PropertyValue::Null => "",
            PropertyValue::String(s) => s.as_str(),
            PropertyValue::Integer(n) => return self.compare_integers_str(left_str, op, *n),
        };

        let result = match op {
            Operator::Eq => left_str == right_str,
            Operator::Ne => left_str != right_str,
            Operator::Lt => left_str < right_str,
            Operator::Le => left_str <= right_str,
            Operator::Gt => left_str > right_str,
            Operator::Ge => left_str >= right_str,
            Operator::Like => left_str.contains(right_str),
            Operator::LeftLike => left_str.starts_with(right_str),
            Operator::RightLike => left_str.ends_with(right_str),
        };

        if result {
            EvalResult::True
        } else {
            EvalResult::False
        }
    }

    fn compare_integers(&self, left: i64, op: &Operator, right: &PropertyValue) -> EvalResult {
        let right_int = match right {
            PropertyValue::Integer(n) => *n,
            PropertyValue::String(s) => s.parse().unwrap_or(0),
            PropertyValue::Null => 0,
        };

        let result = match op {
            Operator::Eq => left == right_int,
            Operator::Ne => left != right_int,
            Operator::Lt => left < right_int,
            Operator::Le => left <= right_int,
            Operator::Gt => left > right_int,
            Operator::Ge => left >= right_int,
            _ => false,
        };

        if result {
            EvalResult::True
        } else {
            EvalResult::False
        }
    }

    fn compare_integers_str(&self, left: &str, op: &Operator, right: i64) -> EvalResult {
        let left_int: i64 = left.parse().unwrap_or(0);
        self.compare_integers(left_int, op, &PropertyValue::Integer(right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> HashMap<String, PropertyValue> {
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
    fn test_simple_property() {
        let p = props(&[("Installed", "1")]);
        assert!(ConditionEvaluator::evaluate("Installed", &p).is_true());

        let p = props(&[]);
        assert!(!ConditionEvaluator::evaluate("Installed", &p).is_true());
    }

    #[test]
    fn test_equality() {
        let p = props(&[("REMOVE", "ALL")]);
        assert!(ConditionEvaluator::evaluate("REMOVE=\"ALL\"", &p).is_true());
        assert!(!ConditionEvaluator::evaluate("REMOVE=\"NONE\"", &p).is_true());
    }

    #[test]
    fn test_not() {
        let p = props(&[]);
        assert!(ConditionEvaluator::evaluate("NOT Installed", &p).is_true());

        let p = props(&[("Installed", "1")]);
        assert!(!ConditionEvaluator::evaluate("NOT Installed", &p).is_true());
    }

    #[test]
    fn test_and_or() {
        let p = props(&[("A", "1"), ("B", "1")]);
        assert!(ConditionEvaluator::evaluate("A AND B", &p).is_true());

        let p = props(&[("A", "1")]);
        assert!(!ConditionEvaluator::evaluate("A AND B", &p).is_true());
        assert!(ConditionEvaluator::evaluate("A OR B", &p).is_true());
    }

    #[test]
    fn test_complex_condition() {
        let p = props(&[("Installed", ""), ("WIX_UPGRADE_DETECTED", "1")]);
        let cond = "NOT Installed AND WIX_UPGRADE_DETECTED";
        assert!(ConditionEvaluator::evaluate(cond, &p).is_true());
    }
}

//! Preprocessor directive preservation
//!
//! WiX files can contain preprocessor directives like `<?if?>`, `<?define?>`, `<?include?>`.
//! These are not valid XML and cause parsers to fail. This module extracts them before
//! parsing and restores them after serialization.

use regex::Regex;
use std::collections::HashMap;

/// Placeholder prefix used to mark extracted preprocessor directives
const PLACEHOLDER_PREFIX: &str = "___WIXER_PP_";

/// Preserves preprocessor directives during XML transformation
#[derive(Debug, Default)]
pub struct PreprocessorPreserver {
    /// Map of placeholder -> original directive
    directives: HashMap<String, String>,
    /// Counter for generating unique placeholders
    counter: usize,
}

impl PreprocessorPreserver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if content contains preprocessor directives
    pub fn has_directives(content: &str) -> bool {
        content.contains("<?if")
            || content.contains("<?else")
            || content.contains("<?endif")
            || content.contains("<?define")
            || content.contains("<?include")
            || content.contains("<?foreach")
            || content.contains("<?endforeach")
    }

    /// Extract preprocessor directives and replace with XML comments containing placeholders
    pub fn extract(&mut self, content: &str) -> String {
        let mut result = content.to_string();

        // Match preprocessor instructions: <?name ...?>
        // We need to handle multi-line directives too
        let re = Regex::new(r#"<\?(if|else|elseif|endif|define|undef|include|foreach|endforeach|error|warning|pragma)\b[^?]*\?>"#).unwrap();

        // Find all matches and replace with placeholders
        let matches: Vec<_> = re.find_iter(content).collect();
        
        // Process in reverse order to preserve positions
        for m in matches.into_iter().rev() {
            let directive = m.as_str();
            let placeholder = format!("{}{}", PLACEHOLDER_PREFIX, self.counter);
            self.directives.insert(placeholder.clone(), directive.to_string());
            self.counter += 1;

            // Replace with an XML comment that won't break parsing
            // We use a special format that can be restored later
            let replacement = format!("<!--{}-->", placeholder);
            result = format!(
                "{}{}{}",
                &result[..m.start()],
                replacement,
                &result[m.end()..]
            );
        }

        result
    }

    /// Restore preprocessor directives from placeholders
    pub fn restore(&self, content: &str) -> String {
        let mut result = content.to_string();

        for (placeholder, directive) in &self.directives {
            // Restore from XML comment format
            let comment_form = format!("<!--{}-->", placeholder);
            result = result.replace(&comment_form, directive);
            
            // Also handle case where it might appear without comment wrapper
            result = result.replace(placeholder, directive);
        }

        result
    }

    /// Get the number of extracted directives
    pub fn directive_count(&self) -> usize {
        self.directives.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_directives() {
        assert!(PreprocessorPreserver::has_directives("<?if $(var.x)?>"));
        assert!(PreprocessorPreserver::has_directives("<?include foo.wxi?>"));
        assert!(!PreprocessorPreserver::has_directives("<Wix xmlns=\"...\">"));
    }

    #[test]
    fn test_extract_and_restore() {
        let mut pp = PreprocessorPreserver::new();
        let input = r#"<Wix>
  <?if $(var.Platform) = x64?>
    <Package Name="x64"/>
  <?else?>
    <Package Name="x86"/>
  <?endif?>
</Wix>"#;

        let extracted = pp.extract(input);
        
        // Should not contain raw preprocessor directives
        assert!(!extracted.contains("<?if"));
        assert!(!extracted.contains("<?else"));
        assert!(!extracted.contains("<?endif"));
        
        // Should contain placeholders in comments
        assert!(extracted.contains(PLACEHOLDER_PREFIX));
        
        // Restore should give back original
        let restored = pp.restore(&extracted);
        assert_eq!(restored, input);
    }

    #[test]
    fn test_extract_include() {
        let mut pp = PreprocessorPreserver::new();
        let input = r#"<Wix>
  <?include Variables.wxi?>
  <Package/>
</Wix>"#;

        let extracted = pp.extract(input);
        assert!(!extracted.contains("<?include"));
        assert_eq!(pp.directive_count(), 1);
        
        let restored = pp.restore(&extracted);
        assert_eq!(restored, input);
    }
}

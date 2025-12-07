//! Individual upgrade transformations
//!
//! Each transformation implements a single v3→v4 change.
//! Transformations are composable and testable in isolation.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use roxmltree::Node;
use std::io::Cursor;

/// WiX v4 namespace constant
pub const V4_NAMESPACE: &'static str = "http://wixtoolset.org/schemas/v4/wxs";

/// Trait for XML node transformations
pub trait Transformation {
    /// Name of this transformation (for logging/debugging)
    fn name(&self) -> &'static str;

    /// Check if this transformation applies to the given node
    fn applies_to(&self, node: &Node) -> bool;

    /// Apply the transformation, writing output to the writer
    /// Returns true if the node was handled, false to continue with default handling
    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool>;
}

// ============================================================================
// Inner Text to Attribute Transformations
// ============================================================================

/// Converts elements where inner text should become a Condition attribute
/// Applies to: Custom, SetProperty, Publish
pub struct InnerTextToCondition;

impl InnerTextToCondition {
    const ELEMENTS: &'static [&'static str] = &["Custom", "SetProperty", "Publish"];
}

impl Transformation for InnerTextToCondition {
    fn name(&self) -> &'static str {
        "InnerTextToCondition"
    }

    fn applies_to(&self, node: &Node) -> bool {
        Self::ELEMENTS.contains(&node.tag_name().name())
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let tag_name = node.tag_name().name();
        let mut elem = BytesStart::new(tag_name);

        // Copy existing attributes
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }

        // Convert inner text to Condition attribute
        let condition_text: String = node
            .children()
            .filter_map(|c| c.text())
            .collect();
        let condition_text = condition_text.trim();

        if !condition_text.is_empty() {
            elem.push_attribute(("Condition", condition_text));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(true)
    }
}

// ============================================================================
// Condition to Launch Transformation
// ============================================================================

/// Converts Condition element (launch condition) to Launch element
pub struct ConditionToLaunch;

impl Transformation for ConditionToLaunch {
    fn name(&self) -> &'static str {
        "ConditionToLaunch"
    }

    fn applies_to(&self, node: &Node) -> bool {
        // In v3, Condition is under Product; in v4 it would be under Package
        // We check for both since we're transforming v3 to v4
        node.tag_name().name() == "Condition"
            && node.parent().map(|p| {
                let name = p.tag_name().name();
                name == "Package" || name == "Product"
            }).unwrap_or(false)
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let message = node.attribute("Message").unwrap_or("Installation cannot continue.");
        
        let condition_text: String = node
            .children()
            .filter_map(|c| c.text())
            .collect();
        let condition_text = condition_text.trim();

        if !condition_text.is_empty() {
            let mut elem = BytesStart::new("Launch");
            elem.push_attribute(("Condition", condition_text));
            elem.push_attribute(("Message", message));
            writer.write_event(Event::Empty(elem))?;
        }

        Ok(true)
    }
}

// ============================================================================
// CustomAction Transformation
// ============================================================================

/// Transforms CustomAction elements
/// - Renames BinaryKey to BinaryRef
/// - Renames FileKey to FileRef
/// - Converts WixCA/WixQuietExec to v4 equivalent
pub struct CustomActionTransform;

impl Transformation for CustomActionTransform {
    fn name(&self) -> &'static str {
        "CustomActionTransform"
    }

    fn applies_to(&self, node: &Node) -> bool {
        node.tag_name().name() == "CustomAction"
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let mut elem = BytesStart::new("CustomAction");

        let binary_key = node.attribute("BinaryKey").or_else(|| node.attribute("BinaryRef"));
        let dll_entry = node.attribute("DllEntry");

        // Check for WixQuietExec pattern
        let is_wix_quiet_exec = binary_key == Some("WixCA")
            && (dll_entry == Some("WixQuietExec") || dll_entry == Some("WixQuietExec64"));

        for attr in node.attributes() {
            let (name, value) = match attr.name() {
                "BinaryKey" | "BinaryRef" if is_wix_quiet_exec => {
                    ("BinaryRef", "Wix4UtilCA_X86")
                }
                "BinaryKey" => ("BinaryRef", attr.value()),
                "FileKey" => ("FileRef", attr.value()),
                other => (other, attr.value()),
            };
            elem.push_attribute((name, value));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(true)
    }
}

// ============================================================================
// Deprecated Attribute Removal
// ============================================================================

/// Removes deprecated attributes from various elements
/// - Win64 attribute (removed in v4, platform is determined by build)
pub struct RemoveDeprecatedAttributes;

impl RemoveDeprecatedAttributes {
    /// Attributes that should be removed entirely in v4
    const DEPRECATED_ATTRS: &'static [&'static str] = &["Win64"];
    
    /// Elements that commonly have these deprecated attributes
    const ELEMENTS: &'static [&'static str] = &[
        "RegistrySearch",
        "Component",
        "CustomAction",
        "ServiceInstall",
    ];
}

impl Transformation for RemoveDeprecatedAttributes {
    fn name(&self) -> &'static str {
        "RemoveDeprecatedAttributes"
    }

    fn applies_to(&self, node: &Node) -> bool {
        let tag = node.tag_name().name();
        Self::ELEMENTS.contains(&tag)
            && node.attributes().any(|a| Self::DEPRECATED_ATTRS.contains(&a.name()))
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let tag_name = node.tag_name().name();
        let mut elem = BytesStart::new(tag_name);

        // Copy attributes, skipping deprecated ones
        for attr in node.attributes() {
            if !Self::DEPRECATED_ATTRS.contains(&attr.name()) {
                elem.push_attribute((attr.name(), attr.value()));
            }
        }

        // Check for children
        let has_children = node.children().any(|c| c.is_element());
        if has_children {
            writer.write_event(Event::Start(elem))?;
            Ok(false) // Let pipeline process children
        } else {
            writer.write_event(Event::Empty(elem))?;
            Ok(true)
        }
    }
}

// ============================================================================
// Factory function
// ============================================================================

/// Create the standard set of transformations
pub fn standard_transformations() -> Vec<Box<dyn Transformation>> {
    vec![
        Box::new(ConditionToLaunch),
        Box::new(InnerTextToCondition),
        Box::new(CustomActionTransform),
        Box::new(RemoveDeprecatedAttributes),
    ]
}

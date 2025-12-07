//! Individual upgrade transformations
//!
//! Each transformation implements a single v3→v4 change.
//! Transformations are composable and testable in isolation.

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use roxmltree::Node;
use std::io::Cursor;

use crate::schema_mapping_v3v4::is_standard_directory;

/// Result type for transformations
pub type TransformResult = anyhow::Result<()>;

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
// Namespace Transformation
// ============================================================================

/// Updates the WiX namespace from v3 to v4
pub struct NamespaceUpgrade;

impl NamespaceUpgrade {
    pub const V3_NAMESPACE: &'static str = "http://schemas.microsoft.com/wix/2006/wi";
    pub const V4_NAMESPACE: &'static str = "http://wixtoolset.org/schemas/v4/wxs";
}

// ============================================================================
// Product to Package Transformation
// ============================================================================

/// Converts Product element to Package element
/// - Renames Product to Package
/// - Moves Id attribute to ProductCode
/// - Merges inner Package element attributes
pub struct ProductToPackage;

impl Transformation for ProductToPackage {
    fn name(&self) -> &'static str {
        "ProductToPackage"
    }

    fn applies_to(&self, node: &Node) -> bool {
        node.tag_name().name() == "Product"
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let mut elem = BytesStart::new("Package");

        // Find inner Package element to merge its attributes
        let inner_package = node.children().find(|c| c.tag_name().name() == "Package");

        // Copy Product attributes, renaming Id to ProductCode
        for attr in node.attributes() {
            let (name, value) = match attr.name() {
                "Id" => ("ProductCode", attr.value()),
                other => (other, attr.value()),
            };
            elem.push_attribute((name, value));
        }

        // Merge inner Package attributes (Scope from InstallScope, etc.)
        if let Some(inner) = inner_package {
            for attr in inner.attributes() {
                let (name, value) = match attr.name() {
                    "InstallScope" => ("Scope", attr.value()),
                    "InstallerVersion" => continue, // Skip, v4 has better defaults
                    "Compressed" => ("Compressed", attr.value()),
                    other => (other, attr.value()),
                };
                // Don't duplicate attributes
                if !node.attributes().any(|a| a.name() == name) {
                    elem.push_attribute((name, value));
                }
            }
        }

        writer.write_event(Event::Start(elem))?;

        // Process children, skipping the inner Package element
        for child in node.children() {
            if child.is_element() && child.tag_name().name() != "Package" {
                // Children will be processed by the pipeline
                // We return false to indicate partial handling
            }
        }

        // Note: We don't close the element here - the pipeline handles that
        Ok(false) // Return false to let pipeline process children
    }
}

// ============================================================================
// Directory Transformations
// ============================================================================

/// Flattens TARGETDIR and converts standard directories to StandardDirectory
pub struct DirectoryTransform;

impl Transformation for DirectoryTransform {
    fn name(&self) -> &'static str {
        "DirectoryTransform"
    }

    fn applies_to(&self, node: &Node) -> bool {
        node.tag_name().name() == "Directory"
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let id = node.attribute("Id").unwrap_or("");

        // Skip TARGETDIR entirely - it's implicit in v4
        if id == "TARGETDIR" {
            // Just process children, don't write this element
            return Ok(false);
        }

        // Convert standard directories
        if is_standard_directory(id) {
            let mut elem = BytesStart::new("StandardDirectory");
            elem.push_attribute(("Id", id));
            // StandardDirectory doesn't need Name attribute
            
            let has_children = node.children().any(|c| c.is_element());
            if has_children {
                writer.write_event(Event::Start(elem))?;
                // Children processed by pipeline
                Ok(false)
            } else {
                writer.write_event(Event::Empty(elem))?;
                Ok(true)
            }
        } else {
            // Regular directory - pass through with all attributes
            Ok(false)
        }
    }
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
// Include Element Transformation
// ============================================================================

/// Adds v4 namespace to Include elements
pub struct IncludeNamespace;

impl Transformation for IncludeNamespace {
    fn name(&self) -> &'static str {
        "IncludeNamespace"
    }

    fn applies_to(&self, node: &Node) -> bool {
        node.tag_name().name() == "Include"
    }

    fn apply(&self, node: &Node, writer: &mut Writer<Cursor<Vec<u8>>>) -> anyhow::Result<bool> {
        let mut elem = BytesStart::new("Include");
        elem.push_attribute(("xmlns", NamespaceUpgrade::V4_NAMESPACE));

        for attr in node.attributes() {
            if attr.name() != "xmlns" {
                elem.push_attribute((attr.name(), attr.value()));
            }
        }

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
        // DirectoryTransform is handled inline in pipeline (needs special closing tag handling)
        // IncludeNamespace is handled specially in pipeline
        // ProductToPackage is handled specially in the pipeline
    ]
}

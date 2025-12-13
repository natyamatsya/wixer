//! Individual upgrade transformations
//!
//! Each transformation implements a single v3→v4 change.
//! Transformations are composable and testable in isolation.

use quick_xml::Writer;
use quick_xml::events::{BytesStart, Event};
use roxmltree::Node;
use std::io::Cursor;

use super::context::UpgradeContext;

/// WiX v4 namespace constant
pub const V4_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxs";

/// WiX v4 UI namespace constant
pub const V4_UI_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxs/ui";

/// Trait for XML node transformations
pub trait Transformation {
    /// Name of this transformation (for logging/debugging)
    fn name(&self) -> &'static str;

    /// Check if this transformation applies to the given node
    fn applies_to(&self, node: &Node, ctx: &UpgradeContext) -> bool;

    /// Apply the transformation, writing output to the writer
    /// Returns true if the node was handled, false to continue with default handling
    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        ctx: &UpgradeContext,
    ) -> anyhow::Result<bool>;
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

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        Self::ELEMENTS.contains(&node.tag_name().name())
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        let tag_name = node.tag_name().name();
        let mut elem = BytesStart::new(tag_name);

        // Copy existing attributes
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }

        // Convert inner text to Condition attribute
        let condition_text: String = node.children().filter_map(|c| c.text()).collect();
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

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        // In v3, Condition is under Product; in v4 it would be under Package
        // We check for both since we're transforming v3 to v4
        node.tag_name().name() == "Condition"
            && node
                .parent()
                .map(|p| {
                    let name = p.tag_name().name();
                    name == "Package" || name == "Product"
                })
                .unwrap_or(false)
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        let message = node
            .attribute("Message")
            .unwrap_or("Installation cannot continue.");

        let condition_text: String = node.children().filter_map(|c| c.text()).collect();
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

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        node.tag_name().name() == "CustomAction"
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        let mut elem = BytesStart::new("CustomAction");

        let binary_key = node
            .attribute("BinaryKey")
            .or_else(|| node.attribute("BinaryRef"));
        let dll_entry = node.attribute("DllEntry");

        // Check for WixQuietExec pattern
        let is_wix_quiet_exec = binary_key == Some("WixCA")
            && (dll_entry == Some("WixQuietExec") || dll_entry == Some("WixQuietExec64"));

        for attr in node.attributes() {
            if attr.name() == "Win64" {
                continue;
            }
            let (name, value) = match attr.name() {
                "BinaryKey" | "BinaryRef" if is_wix_quiet_exec => ("BinaryRef", "Wix4UtilCA_X86"),
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

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        let tag = node.tag_name().name();
        Self::ELEMENTS.contains(&tag)
            && node
                .attributes()
                .any(|a| Self::DEPRECATED_ATTRS.contains(&a.name()))
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
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
// UI Control Text Inner to Attribute
// ============================================================================

/// Converts Control elements where <Text>inner</Text> should become Text="value" attribute
/// WiX v4 error: WIX0400: The Text element contains inner text which is obsolete.
pub struct ControlTextToAttribute;

impl Transformation for ControlTextToAttribute {
    fn name(&self) -> &'static str {
        "ControlTextToAttribute"
    }

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        if node.tag_name().name() != "Control" {
            return false;
        }
        // Check if there's a Text child element with inner text
        node.children().any(|c| {
            c.tag_name().name() == "Text"
                && c.children()
                    .any(|t| t.is_text() && !t.text().unwrap_or("").trim().is_empty())
        })
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        let mut elem = BytesStart::new("Control");

        // Copy existing attributes
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }

        // Find Text child and extract inner text
        for child in node.children() {
            if child.tag_name().name() == "Text"
                && let Some(text) = child.text()
                && !text.trim().is_empty()
            {
                elem.push_attribute(("Text", text.trim()));
            }
        }

        let mut enable_condition: Option<String> = None;
        let mut disable_condition: Option<String> = None;

        for child in node.children() {
            if child.tag_name().name() == "Condition" {
                let action = child.attribute("Action").unwrap_or("");
                let condition_text: String = child.children().filter_map(|c| c.text()).collect();
                let condition_text = condition_text.trim().to_string();

                match action.to_lowercase().as_str() {
                    "enable" => enable_condition = Some(condition_text),
                    "disable" => disable_condition = Some(condition_text),
                    _ => {}
                }
            }
        }

        if let Some(cond) = disable_condition {
            elem.push_attribute(("DisableCondition", cond.as_str()));
        }
        if let Some(cond) = enable_condition {
            elem.push_attribute(("EnableCondition", cond.as_str()));
        }

        // Check for other children (non-Text elements)
        let has_other_children = node.children().any(|c| {
            c.is_element() && c.tag_name().name() != "Text" && c.tag_name().name() != "Condition"
        });

        if has_other_children {
            writer.write_event(Event::Start(elem))?;
            Ok(false) // Let pipeline process remaining children (skip Text elements)
        } else {
            writer.write_event(Event::Empty(elem))?;
            Ok(true)
        }
    }
}

// ============================================================================
// UI Control Condition to Attributes
// ============================================================================

/// Converts Control/Condition child elements to EnableCondition/DisableCondition attributes
/// WiX v4 error: WIX0005: The Control element contains an unexpected child element 'Condition'.
pub struct ControlConditionToAttribute;

impl Transformation for ControlConditionToAttribute {
    fn name(&self) -> &'static str {
        "ControlConditionToAttribute"
    }

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        if node.tag_name().name() != "Control" {
            return false;
        }
        // Check if there's a Condition child element
        node.children().any(|c| c.tag_name().name() == "Condition")
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        let mut elem = BytesStart::new("Control");

        // Copy existing attributes
        for attr in node.attributes() {
            elem.push_attribute((attr.name(), attr.value()));
        }

        // Find Condition children and convert to attributes
        let mut enable_condition: Option<String> = None;
        let mut disable_condition: Option<String> = None;

        for child in node.children() {
            if child.tag_name().name() == "Condition" {
                let action = child.attribute("Action").unwrap_or("");
                let condition_text: String = child.children().filter_map(|c| c.text()).collect();
                let condition_text = condition_text.trim().to_string();

                match action.to_lowercase().as_str() {
                    "enable" => enable_condition = Some(condition_text),
                    "disable" => disable_condition = Some(condition_text),
                    _ => {} // Ignore other actions like "show", "hide", "default"
                }
            }
        }

        if let Some(cond) = disable_condition {
            elem.push_attribute(("DisableCondition", cond.as_str()));
        }
        if let Some(cond) = enable_condition {
            elem.push_attribute(("EnableCondition", cond.as_str()));
        }

        // Check for other children (non-Condition elements)
        let has_other_children = node
            .children()
            .any(|c| c.is_element() && c.tag_name().name() != "Condition");

        if has_other_children {
            writer.write_event(Event::Start(elem))?;
            Ok(false) // Let pipeline process remaining children (skip Condition elements)
        } else {
            writer.write_event(Event::Empty(elem))?;
            Ok(true)
        }
    }
}

// ============================================================================
// Skip Text and Condition children under Control
// ============================================================================

/// Skips Text and Condition elements that are children of Control
/// (they've been converted to attributes by other transformations)
pub struct SkipControlTextAndCondition;

impl Transformation for SkipControlTextAndCondition {
    fn name(&self) -> &'static str {
        "SkipControlTextAndCondition"
    }

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        let tag = node.tag_name().name();
        if tag != "Text" && tag != "Condition" {
            return false;
        }
        // Only skip if parent is Control
        node.parent()
            .map(|p| p.tag_name().name() == "Control")
            .unwrap_or(false)
    }

    fn apply(
        &self,
        _node: &Node,
        _writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        // Skip this element entirely - it's been converted to an attribute
        Ok(true)
    }
}

// ============================================================================
// UIRef to ui:WixUI Transformation
// ============================================================================

/// Converts UIRef elements referencing WixUI_* to ui:WixUI elements
/// WiX v4: UIRef Id="WixUI_InstallDir" → ui:WixUI Id="WixUI_InstallDir" InstallDirectory="..."
/// Uses cross-file context to find WIXUI_INSTALLDIR property value
pub struct UIRefToWixUI;

impl Transformation for UIRefToWixUI {
    fn name(&self) -> &'static str {
        "UIRefToWixUI"
    }

    fn applies_to(&self, node: &Node, _ctx: &UpgradeContext) -> bool {
        if node.tag_name().name() != "UIRef" {
            return false;
        }
        node.attribute("Id")
            .map(|id| id.starts_with("WixUI_"))
            .unwrap_or(false)
    }

    fn apply(
        &self,
        node: &Node,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        let id = node.attribute("Id").unwrap_or("WixUI_Minimal");

        let mut elem = BytesStart::new("ui:WixUI");
        elem.push_attribute(("Id", id));

        // For WixUI_InstallDir, get WIXUI_INSTALLDIR from cross-file context
        if id == "WixUI_InstallDir" {
            // First try cross-file context (most reliable)
            if let Some(install_dir) = ctx.get_wixui_install_directory() {
                elem.push_attribute(("InstallDirectory", install_dir));
            } else {
                // Fallback: try to find in same file (siblings)
                if let Some(parent) = node.parent() {
                    for sibling in parent.children() {
                        if sibling.tag_name().name() == "Property"
                            && sibling.attribute("Id") == Some("WIXUI_INSTALLDIR")
                            && let Some(value) = sibling.attribute("Value")
                        {
                            elem.push_attribute(("InstallDirectory", value));
                            break;
                        }
                    }
                }
            }
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(true)
    }
}

// ============================================================================
// Skip WIXUI_INSTALLDIR Property (now set by ui:WixUI)
// ============================================================================

/// Skips WIXUI_INSTALLDIR Property element when UIRef is converted to ui:WixUI
/// (the InstallDirectory is now an attribute on ui:WixUI)
/// Uses cross-file context to detect if WixUI_InstallDir is used anywhere
pub struct SkipWixUIInstallDirProperty;

impl Transformation for SkipWixUIInstallDirProperty {
    fn name(&self) -> &'static str {
        "SkipWixUIInstallDirProperty"
    }

    fn applies_to(&self, node: &Node, ctx: &UpgradeContext) -> bool {
        if node.tag_name().name() != "Property" {
            return false;
        }
        if node.attribute("Id") != Some("WIXUI_INSTALLDIR") {
            return false;
        }
        // Skip if WixUI_InstallDir is used anywhere (cross-file context)
        if ctx.has_ui_ref("WixUI_InstallDir") {
            return true;
        }
        // Fallback: check same parent for UIRef
        if let Some(parent) = node.parent() {
            return parent.children().any(|sibling| {
                sibling.tag_name().name() == "UIRef"
                    && sibling.attribute("Id") == Some("WixUI_InstallDir")
            });
        }
        false
    }

    fn apply(
        &self,
        _node: &Node,
        _writer: &mut Writer<Cursor<Vec<u8>>>,
        _ctx: &UpgradeContext,
    ) -> anyhow::Result<bool> {
        // Skip this element - it's been converted to an attribute on ui:WixUI
        Ok(true)
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
        // UI transformations
        Box::new(ControlTextToAttribute),
        Box::new(ControlConditionToAttribute),
        Box::new(SkipControlTextAndCondition),
        Box::new(UIRefToWixUI),
        Box::new(SkipWixUIInstallDirProperty),
    ]
}

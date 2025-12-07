//! WiX v3 to v4 Schema Mapping
//!
//! This module provides systematic mappings for upgrading WiX v3 projects to v4.
//! Based on the official WiX v4 migration guide.
//!
//! See also:
//! - `schema_mapping.rs` — WiX ↔ MSIX schema mapping for migration

use std::collections::HashMap;

/// Element rename mappings from v3 to v4
pub fn element_renames() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    // Product + Package merge into Package
    map.insert("Product", "Package");
    // Module stays Module but structure changes
    map
}

/// Attribute rename mappings (element, old_attr) -> new_attr
pub fn attribute_renames() -> HashMap<(&'static str, &'static str), &'static str> {
    let mut map = HashMap::new();
    // Product/@Id -> Package/@ProductCode (when not "*")
    map.insert(("Product", "Id"), "ProductCode");
    // Package/@InstallScope -> Package/@Scope
    map.insert(("Package", "InstallScope"), "Scope");
    // CustomAction/@BinaryKey -> CustomAction/@BinaryRef
    map.insert(("CustomAction", "BinaryKey"), "BinaryRef");
    // CustomAction/@FileKey -> CustomAction/@FileRef  
    map.insert(("CustomAction", "FileKey"), "FileRef");
    // CustomAction/@Property -> CustomAction/@Property (stays same but usage changes)
    map
}

/// Elements that change structure significantly in v4
pub fn structural_element_changes() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    // Condition under Product -> Launch conditions handled differently
    // Custom inner text -> Custom/@Condition attribute
    map.insert("Custom", "Condition attribute instead of inner text");
    map.insert("Condition", "Use Launch element with Condition/Message attributes");
    map.insert("Publish", "Condition attribute instead of inner text");
    map
}

/// WiX extensions and their v4 package names
pub fn extension_packages() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("WixUtilExtension", "WixToolset.Util.wixext");
    map.insert("WixUIExtension", "WixToolset.UI.wixext");
    map.insert("WixNetFxExtension", "WixToolset.Netfx.wixext");
    map.insert("WixIIsExtension", "WixToolset.Iis.wixext");
    map.insert("WixFirewallExtension", "WixToolset.Firewall.wixext");
    map.insert("WixSqlExtension", "WixToolset.Sql.wixext");
    map.insert("WixVSExtension", "WixToolset.VisualStudio.wixext");
    map.insert("WixBalExtension", "WixToolset.Bal.wixext");
    map.insert("WixDependencyExtension", "WixToolset.Dependency.wixext");
    map
}

/// Detect which extensions are needed based on element/attribute usage
pub fn detect_required_extensions(content: &str) -> Vec<&'static str> {
    let mut extensions = Vec::new();
    
    // WixCA binary -> Util extension
    if content.contains("WixCA") || content.contains("WixQuietExec") {
        extensions.push("WixToolset.Util.wixext");
    }
    
    // UI references
    if content.contains("WixUI_") || content.contains("UIRef") {
        extensions.push("WixToolset.UI.wixext");
    }
    
    // IIS elements
    if content.contains("iis:") || content.contains("WebSite") || content.contains("WebApplication") {
        extensions.push("WixToolset.Iis.wixext");
    }
    
    // .NET Framework
    if content.contains("netfx:") || content.contains("NetFx") {
        extensions.push("WixToolset.Netfx.wixext");
    }
    
    extensions.sort();
    extensions.dedup();
    extensions
}

/// Attributes to remove during upgrade
pub fn removed_attributes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Package", "InstallerVersion"), // Now has better defaults
        ("Package", "Keywords"),
        ("Package", "Description"),
        ("Package", "Comments"),
        ("Package", "Platform"), // Use -arch flag instead
    ]
}

/// Standard directories that should use StandardDirectory element
pub const STANDARD_DIRECTORIES: &[&str] = &[
    "TARGETDIR",
    "ProgramFilesFolder",
    "ProgramFiles64Folder", 
    "ProgramFiles6432Folder",
    "CommonFilesFolder",
    "CommonFiles64Folder",
    "CommonFiles6432Folder",
    "SystemFolder",
    "System64Folder",
    "System6432Folder",
    "WindowsFolder",
    "TempFolder",
    "LocalAppDataFolder",
    "AppDataFolder",
    "CommonAppDataFolder",
    "DesktopFolder",
    "StartMenuFolder",
    "ProgramMenuFolder",
    "StartupFolder",
    "FontsFolder",
    "SendToFolder",
    "FavoritesFolder",
    "NetHoodFolder",
    "PrintHoodFolder",
    "RecentFolder",
    "TemplateFolder",
    "AdminToolsFolder",
    "PersonalFolder",
    "MyPicturesFolder",
];

/// Check if a directory ID is a standard directory
pub fn is_standard_directory(id: &str) -> bool {
    STANDARD_DIRECTORIES.contains(&id)
}

/// Namespace changes
pub const V3_NAMESPACE: &str = "http://schemas.microsoft.com/wix/2006/wi";
pub const V4_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxs";

/// Extension namespace mappings
pub fn extension_namespaces() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("http://schemas.microsoft.com/wix/UtilExtension", "http://wixtoolset.org/schemas/v4/wxs/util");
    map.insert("http://schemas.microsoft.com/wix/IIsExtension", "http://wixtoolset.org/schemas/v4/wxs/iis");
    map.insert("http://schemas.microsoft.com/wix/NetFxExtension", "http://wixtoolset.org/schemas/v4/wxs/netfx");
    map.insert("http://schemas.microsoft.com/wix/FirewallExtension", "http://wixtoolset.org/schemas/v4/wxs/firewall");
    map.insert("http://schemas.microsoft.com/wix/SqlExtension", "http://wixtoolset.org/schemas/v4/wxs/sql");
    map.insert("http://schemas.microsoft.com/wix/VSExtension", "http://wixtoolset.org/schemas/v4/wxs/vs");
    map.insert("http://schemas.microsoft.com/wix/BalExtension", "http://wixtoolset.org/schemas/v4/wxs/bal");
    map
}

/// Elements that were removed or significantly changed
pub fn deprecated_elements() -> Vec<&'static str> {
    vec![
        "PropertyRef", // Often not needed in v4
    ]
}

/// Elements that need structural changes
#[derive(Debug, Clone)]
pub enum StructuralChange {
    /// Directory with standard ID should become StandardDirectory
    DirectoryToStandardDirectory,
    /// Nested standard directories should be flattened
    FlattenStandardDirectories,
    /// Product + Package merge
    MergeProductPackage,
}

/// Get structural changes needed for an element
pub fn get_structural_changes(element: &str, id: Option<&str>) -> Vec<StructuralChange> {
    let mut changes = Vec::new();
    
    match element {
        "Directory" => {
            if let Some(id) = id {
                if is_standard_directory(id) {
                    changes.push(StructuralChange::DirectoryToStandardDirectory);
                }
            }
        }
        "Product" => {
            changes.push(StructuralChange::MergeProductPackage);
        }
        _ => {}
    }
    
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_directories() {
        assert!(is_standard_directory("TARGETDIR"));
        assert!(is_standard_directory("ProgramFilesFolder"));
        assert!(!is_standard_directory("INSTALLFOLDER"));
        assert!(!is_standard_directory("MyCustomDir"));
    }

    #[test]
    fn test_namespace_mapping() {
        let ns = extension_namespaces();
        assert_eq!(
            ns.get("http://schemas.microsoft.com/wix/UtilExtension"),
            Some(&"http://wixtoolset.org/schemas/v4/wxs/util")
        );
    }
}

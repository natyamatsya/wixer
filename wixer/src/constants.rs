/// WiX XML Namespaces
pub const WIX_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxs";
pub const WIX_LOCALIZATION_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxl";

macro_rules! define_extensions {
    ($($base:ident, $yaml:ident, $literal:literal);* $(;)?) => {
        $(
            pub const $base: &str = $literal;
            pub const $yaml: &str = concat!($literal, ".yaml");
        )*

        pub const ALL_SOURCE_EXTENSIONS: &[&str] = &[
            $($yaml),*
        ];

        pub const ALL_TARGET_EXTENSIONS: &[&str] = &[
            $($literal),*
        ];
    };
}

// Define base and YAML extensions
define_extensions! {
    EXT_WXS, EXT_WXS_YAML, "wxs";
    EXT_WXI, EXT_WXI_YAML, "wxi";
    EXT_WXL, EXT_WXL_YAML, "wxl";
    EXT_WSI, EXT_WSI_YAML, "wsi";
    EXT_WIXLOC, EXT_WIXLOC_YAML, "wixloc";
}

// Other extensions
pub const EXT_WIXPDB: &str = "wixpdb";
pub const EXT_MSI: &str = "msi";

/// Directory and File Names
pub const WIXER_DIR: &str = ".wixer";
pub const INTERMEDIATES_DIR: &str = "intermediates";
pub const KNOWN_ATTRIBUTES_FILE: &str = "known_attributes.json";
pub const SCHEMA_DIR_NAME: &str = "json-schema";
pub const DOWNLOADED_SCHEMAS_DIR: &str = "schemas";
pub const GENERATED_SCHEMAS_DIR: &str = "json-schemas";

/// YAML Conversion Constants
pub const TEXT_PROPERTIES: &[&str] = &["_value", "value", "text"];
pub const COMMENT_PROPERTY: &str = "_comment";
pub const ATTRIBUTES_PROPERTY: &str = "attributes";
pub const ELEMENTS_PROPERTY: &str = "elements";

/// CPack specific variables
pub const CPACK_VAR_KEY: &str = "CPACK_WIX_CUSTOM_XMLNS_EXPANDED";
pub const CPACK_VAR_TOKEN: &str = "@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@";
pub const CPACK_VAR_XML: &str = " CPACK_WIX_CUSTOM_XMLNS_EXPANDED=\"1\"";

/// MSI Standard Properties
pub mod msi_properties {
    // Installation state properties
    pub const INSTALLED: &str = "Installed";
    pub const REINSTALL: &str = "REINSTALL";
    pub const REMOVE: &str = "REMOVE";

    // Upgrade properties
    pub const WIX_UPGRADE_DETECTED: &str = "WIX_UPGRADE_DETECTED";
    pub const WIX_DOWNGRADE_DETECTED: &str = "WIX_DOWNGRADE_DETECTED";
    pub const UPGRADINGPRODUCTCODE: &str = "UPGRADINGPRODUCTCODE";

    // Common values
    pub const ALL: &str = "ALL";
}

/// WiX XML Element and Attribute Names
pub mod wix_elements {
    // Core container elements
    pub const WIX: &str = "Wix";
    pub const PACKAGE: &str = "Package";
    pub const FRAGMENT: &str = "Fragment";
    pub const MODULE: &str = "Module";

    // Component elements
    pub const COMPONENT: &str = "Component";
    pub const COMPONENT_GROUP: &str = "ComponentGroup";
    pub const COMPONENT_REF: &str = "ComponentRef";
    pub const COMPONENT_GROUP_REF: &str = "ComponentGroupRef";

    // Feature elements
    pub const FEATURE: &str = "Feature";
    pub const FEATURE_REF: &str = "FeatureRef";
    pub const FEATURE_GROUP: &str = "FeatureGroup";
    pub const FEATURE_GROUP_REF: &str = "FeatureGroupRef";

    // Directory elements
    pub const DIRECTORY: &str = "Directory";
    pub const DIRECTORY_REF: &str = "DirectoryRef";
    pub const STANDARD_DIRECTORY: &str = "StandardDirectory";

    // File elements
    pub const FILE: &str = "File";
    pub const FILE_REF: &str = "FileRef";

    // UI elements
    pub const UI: &str = "UI";
    pub const DIALOG: &str = "Dialog";
    pub const CONTROL: &str = "Control";
    pub const PUBLISH: &str = "Publish";

    // Custom action elements
    pub const CUSTOM_ACTION: &str = "CustomAction";
    pub const CUSTOM: &str = "Custom";
    pub const BINARY: &str = "Binary";
    pub const BINARY_REF: &str = "BinaryRef";

    // Property elements
    pub const PROPERTY: &str = "Property";
    pub const PROPERTY_REF: &str = "PropertyRef";
    pub const SET_PROPERTY: &str = "SetProperty";

    // Common attributes
    pub const ID: &str = "Id";
    pub const NAME: &str = "Name";
    pub const VALUE: &str = "Value";
    pub const EVENT: &str = "Event";
    pub const ORDER: &str = "Order";
    pub const CONDITION: &str = "Condition";
}

/// WiX UI Publish Events
pub mod ui_events {
    // Navigation events - change dialog flow
    pub const NEW_DIALOG: &str = "NewDialog";
    pub const SPAWN_DIALOG: &str = "SpawnDialog";
    pub const SPAWN_WAIT_DIALOG: &str = "SpawnWaitDialog";
    pub const END_DIALOG: &str = "EndDialog";

    // Action events - perform operations
    pub const DO_ACTION: &str = "DoAction";
    pub const SET_PROPERTY: &str = "SetProperty";
    pub const SET_TARGET_PATH: &str = "SetTargetPath";
    pub const CHECK_TARGET_PATH: &str = "CheckTargetPath";
    pub const CHECK_EXISTING_TARGET_PATH: &str = "CheckExistingTargetPath";
    pub const RESET: &str = "Reset";
    pub const REINSTALL_MODE: &str = "ReinstallMode";
    pub const REINSTALL: &str = "Reinstall";
    pub const VALIDATE_PRODUCT_ID: &str = "ValidateProductID";

    // Selection events - feature/component selection
    pub const ADD_LOCAL: &str = "AddLocal";
    pub const ADD_SOURCE: &str = "AddSource";
    pub const REMOVE: &str = "Remove";
    pub const SELECT_FEATURE: &str = "SelectionBrowse";

    // UI control events
    pub const SET_INSTALL_LEVEL: &str = "SetInstallLevel";
    pub const ENABLE: &str = "Enable";
    pub const DISABLE: &str = "Disable";
    pub const SHOW: &str = "Show";
    pub const HIDE: &str = "Hide";

    /// Navigation events that change dialog flow
    pub const NAVIGATION_EVENTS: &[&str] =
        &[NEW_DIALOG, SPAWN_DIALOG, SPAWN_WAIT_DIALOG, END_DIALOG];

    /// Action events that perform operations
    pub const ACTION_EVENTS: &[&str] = &[
        DO_ACTION,
        SET_PROPERTY,
        SET_TARGET_PATH,
        CHECK_TARGET_PATH,
        CHECK_EXISTING_TARGET_PATH,
    ];
}

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

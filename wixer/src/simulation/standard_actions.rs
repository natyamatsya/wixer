//! Standard Windows Installer actions with their default sequence numbers
//!
//! Reference: https://docs.microsoft.com/en-us/windows/win32/msi/standard-actions

use super::model::{Action, Sequence, SequenceTable};

/// Standard action categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCategory {
    /// File costing actions
    Costing,
    /// User interface actions
    UserInterface,
    /// File operations
    FileOps,
    /// Registry operations
    Registry,
    /// Service operations
    Services,
    /// Installation control
    InstallControl,
    /// Upgrade handling
    Upgrade,
    /// System operations
    System,
}

/// Standard action definition
pub struct StandardAction {
    pub name: &'static str,
    pub ui_sequence: Option<i32>,
    pub execute_sequence: Option<i32>,
    pub category: ActionCategory,
    pub description: &'static str,
}

/// All standard Windows Installer actions
pub const STANDARD_ACTIONS: &[StandardAction] = &[
    // Costing actions
    StandardAction {
        name: "CostInitialize",
        ui_sequence: Some(800),
        execute_sequence: Some(800),
        category: ActionCategory::Costing,
        description: "Initiates the internal installation costing process",
    },
    StandardAction {
        name: "FileCost",
        ui_sequence: Some(900),
        execute_sequence: Some(900),
        category: ActionCategory::Costing,
        description: "Initiates dynamic costing of standard installation actions",
    },
    StandardAction {
        name: "CostFinalize",
        ui_sequence: Some(1000),
        execute_sequence: Some(1000),
        category: ActionCategory::Costing,
        description: "Ends the internal installation costing process",
    },
    // Search actions
    StandardAction {
        name: "AppSearch",
        ui_sequence: Some(50),
        execute_sequence: Some(50),
        category: ActionCategory::System,
        description: "Searches for existing applications using file/registry signatures",
    },
    StandardAction {
        name: "CCPSearch",
        ui_sequence: Some(500),
        execute_sequence: None,
        category: ActionCategory::System,
        description: "Searches for products using Compliance Checking Program",
    },
    StandardAction {
        name: "RMCCPSearch",
        ui_sequence: Some(600),
        execute_sequence: None,
        category: ActionCategory::System,
        description: "Searches for products listed in the CCPSearch table using file signatures",
    },
    StandardAction {
        name: "LaunchConditions",
        ui_sequence: Some(100),
        execute_sequence: Some(100),
        category: ActionCategory::InstallControl,
        description: "Evaluates launch conditions and terminates if not met",
    },
    // Upgrade actions
    StandardAction {
        name: "FindRelatedProducts",
        ui_sequence: Some(25),
        execute_sequence: Some(25),
        category: ActionCategory::Upgrade,
        description: "Searches for related products via UpgradeCode",
    },
    StandardAction {
        name: "MigrateFeatureStates",
        ui_sequence: Some(1200),
        execute_sequence: Some(1200),
        category: ActionCategory::Upgrade,
        description: "Migrates feature states from related products",
    },
    StandardAction {
        name: "RemoveExistingProducts",
        ui_sequence: None,
        execute_sequence: Some(6700),
        category: ActionCategory::Upgrade,
        description: "Removes related products during major upgrade",
    },
    // Validation
    StandardAction {
        name: "InstallValidate",
        ui_sequence: Some(1400),
        execute_sequence: Some(1400),
        category: ActionCategory::InstallControl,
        description: "Validates sufficient disk space for installation",
    },
    // Installation control
    StandardAction {
        name: "InstallInitialize",
        ui_sequence: None,
        execute_sequence: Some(1500),
        category: ActionCategory::InstallControl,
        description: "Marks beginning of installation script",
    },
    StandardAction {
        name: "InstallFinalize",
        ui_sequence: None,
        execute_sequence: Some(6600),
        category: ActionCategory::InstallControl,
        description: "Marks end of installation script, commits or rolls back",
    },
    StandardAction {
        name: "ProcessComponents",
        ui_sequence: None,
        execute_sequence: Some(1600),
        category: ActionCategory::InstallControl,
        description: "Registers/unregisters components",
    },
    StandardAction {
        name: "UnpublishComponents",
        ui_sequence: None,
        execute_sequence: Some(1700),
        category: ActionCategory::InstallControl,
        description: "Unpublishes qualified components",
    },
    StandardAction {
        name: "PublishComponents",
        ui_sequence: None,
        execute_sequence: Some(6200),
        category: ActionCategory::InstallControl,
        description: "Publishes qualified components",
    },
    // File operations
    StandardAction {
        name: "InstallFiles",
        ui_sequence: None,
        execute_sequence: Some(4000),
        category: ActionCategory::FileOps,
        description: "Copies files from source to destination",
    },
    StandardAction {
        name: "RemoveFiles",
        ui_sequence: None,
        execute_sequence: Some(3500),
        category: ActionCategory::FileOps,
        description: "Removes files previously installed",
    },
    StandardAction {
        name: "DuplicateFiles",
        ui_sequence: None,
        execute_sequence: Some(4210),
        category: ActionCategory::FileOps,
        description: "Duplicates files installed by InstallFiles",
    },
    StandardAction {
        name: "MoveFiles",
        ui_sequence: None,
        execute_sequence: Some(3800),
        category: ActionCategory::FileOps,
        description: "Moves files on the target machine",
    },
    StandardAction {
        name: "PatchFiles",
        ui_sequence: None,
        execute_sequence: Some(4090),
        category: ActionCategory::FileOps,
        description: "Applies patches to installed files",
    },
    StandardAction {
        name: "RemoveDuplicateFiles",
        ui_sequence: None,
        execute_sequence: Some(3400),
        category: ActionCategory::FileOps,
        description: "Removes duplicate files created by DuplicateFiles",
    },
    // Directories
    StandardAction {
        name: "CreateFolders",
        ui_sequence: None,
        execute_sequence: Some(3700),
        category: ActionCategory::FileOps,
        description: "Creates empty folders for components",
    },
    StandardAction {
        name: "RemoveFolders",
        ui_sequence: None,
        execute_sequence: Some(3600),
        category: ActionCategory::FileOps,
        description: "Removes empty folders",
    },
    // Registry
    StandardAction {
        name: "WriteRegistryValues",
        ui_sequence: None,
        execute_sequence: Some(5000),
        category: ActionCategory::Registry,
        description: "Writes registry values from Registry table",
    },
    StandardAction {
        name: "RemoveRegistryValues",
        ui_sequence: None,
        execute_sequence: Some(2600),
        category: ActionCategory::Registry,
        description: "Removes registry values",
    },
    StandardAction {
        name: "WriteIniValues",
        ui_sequence: None,
        execute_sequence: Some(5100),
        category: ActionCategory::Registry,
        description: "Writes .ini file values",
    },
    StandardAction {
        name: "RemoveIniValues",
        ui_sequence: None,
        execute_sequence: Some(3100),
        category: ActionCategory::Registry,
        description: "Removes .ini file values",
    },
    // Shortcuts
    StandardAction {
        name: "CreateShortcuts",
        ui_sequence: None,
        execute_sequence: Some(4500),
        category: ActionCategory::FileOps,
        description: "Creates shortcuts",
    },
    StandardAction {
        name: "RemoveShortcuts",
        ui_sequence: None,
        execute_sequence: Some(3200),
        category: ActionCategory::FileOps,
        description: "Removes shortcuts",
    },
    // Services
    StandardAction {
        name: "InstallServices",
        ui_sequence: None,
        execute_sequence: Some(5800),
        category: ActionCategory::Services,
        description: "Registers services",
    },
    StandardAction {
        name: "DeleteServices",
        ui_sequence: None,
        execute_sequence: Some(2000),
        category: ActionCategory::Services,
        description: "Deletes services",
    },
    StandardAction {
        name: "StartServices",
        ui_sequence: None,
        execute_sequence: Some(5900),
        category: ActionCategory::Services,
        description: "Starts services",
    },
    StandardAction {
        name: "StopServices",
        ui_sequence: None,
        execute_sequence: Some(1900),
        category: ActionCategory::Services,
        description: "Stops services",
    },
    // Registration
    StandardAction {
        name: "RegisterProduct",
        ui_sequence: None,
        execute_sequence: Some(6100),
        category: ActionCategory::System,
        description: "Registers product in Add/Remove Programs",
    },
    StandardAction {
        name: "RegisterUser",
        ui_sequence: None,
        execute_sequence: Some(6000),
        category: ActionCategory::System,
        description: "Registers user information",
    },
    StandardAction {
        name: "UnpublishFeatures",
        ui_sequence: None,
        execute_sequence: Some(1800),
        category: ActionCategory::System,
        description: "Unpublishes features from system",
    },
    StandardAction {
        name: "PublishFeatures",
        ui_sequence: None,
        execute_sequence: Some(6300),
        category: ActionCategory::System,
        description: "Publishes features to system",
    },
    StandardAction {
        name: "PublishProduct",
        ui_sequence: None,
        execute_sequence: Some(6400),
        category: ActionCategory::System,
        description: "Publishes product to system",
    },
    // UI actions
    StandardAction {
        name: "ExecuteAction",
        ui_sequence: Some(1300),
        execute_sequence: None,
        category: ActionCategory::UserInterface,
        description: "Initiates execute sequence from UI sequence",
    },
];

/// Get a standard action by name
pub fn get_standard_action(name: &str) -> Option<&'static StandardAction> {
    STANDARD_ACTIONS.iter().find(|a| a.name == name)
}

/// Check if an action name is a standard action
pub fn is_standard_action(name: &str) -> bool {
    get_standard_action(name).is_some()
}

/// Build a default InstallExecuteSequence with standard actions
pub fn default_execute_sequence() -> Sequence {
    let mut seq = Sequence::new(SequenceTable::InstallExecuteSequence);

    for sa in STANDARD_ACTIONS {
        if let Some(seq_num) = sa.execute_sequence {
            let mut action = Action::new_standard(sa.name, seq_num);
            action.description = Some(sa.description.to_string());
            seq.add_action(action);
        }
    }

    seq
}

/// Build a default InstallUISequence with standard actions
pub fn default_ui_sequence() -> Sequence {
    let mut seq = Sequence::new(SequenceTable::InstallUISequence);

    for sa in STANDARD_ACTIONS {
        if let Some(seq_num) = sa.ui_sequence {
            let mut action = Action::new_standard(sa.name, seq_num);
            action.description = Some(sa.description.to_string());
            seq.add_action(action);
        }
    }

    seq
}

/// Key sequence boundaries
pub mod boundaries {
    /// Before FindRelatedProducts
    pub const EARLY: i32 = 20;
    /// After AppSearch
    pub const AFTER_APPSEARCH: i32 = 60;
    /// After CostFinalize
    pub const AFTER_COSTING: i32 = 1010;
    /// After InstallValidate
    pub const AFTER_VALIDATE: i32 = 1410;
    /// After InstallInitialize
    pub const AFTER_INSTALL_INIT: i32 = 1510;
    /// Before InstallFinalize
    pub const BEFORE_INSTALL_FINAL: i32 = 6590;
    /// After RemoveFiles (for uninstall actions)
    pub const AFTER_REMOVE_FILES: i32 = 3510;
}

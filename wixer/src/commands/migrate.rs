use crate::common::AttributeStrategy;
use crate::constants::{EXT_WXI_YAML, EXT_WXL_YAML, EXT_WXS_YAML};
use crate::project::WixProject;
use crate::utils::load_known_attributes;
use crate::yaml_to_wix;
use anyhow::{Context, Result};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum MigrationTarget {
    Msix,
}

pub fn run(input: PathBuf, output_dir: PathBuf, target: MigrationTarget) -> Result<()> {
    match target {
        MigrationTarget::Msix => migrate_to_msix(&input, &output_dir),
    }
}

fn migrate_to_msix(input: &Path, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    let is_template = input.extension().is_some_and(|e| e == "in");

    // Check if input is YAML - if so, convert to temp XML first
    let input_str = input.to_string_lossy();
    let is_yaml = input_str.ends_with(EXT_WXS_YAML)
        || input_str.ends_with(EXT_WXI_YAML)
        || input_str.ends_with(EXT_WXL_YAML);

    let temp_dir = if is_yaml {
        let temp = output_dir.join("wixer_temp");
        fs::create_dir_all(&temp)?;
        Some(temp)
    } else {
        None
    };

    // Load Project - either from converted YAML or directly from XML
    let mut project = WixProject::new();

    if is_yaml {
        // Convert YAML to XML in temp directory
        let temp_dir = temp_dir.as_ref().unwrap();
        let known_attributes = load_known_attributes();

        // Convert the input file
        let xml_filename = input
            .file_name()
            .map(|n| n.to_string_lossy().replace(".yaml", ""))
            .unwrap_or_else(|| "converted.wxs".to_string());
        let xml_path = temp_dir.join(&xml_filename);

        yaml_to_wix::convert_file(
            input,
            &xml_path,
            AttributeStrategy::Inline,
            &known_attributes,
        )
        .with_context(|| format!("Failed to convert YAML file: {}", input.display()))?;

        info!("Converted YAML to XML: {}", xml_path.display());

        // Also convert any other YAML files in the same directory
        if let Some(parent) = input.parent()
            && parent.exists()
        {
            for entry in fs::read_dir(parent)? {
                let entry = entry?;
                let path = entry.path();
                let path_str = path.to_string_lossy();

                if path != input
                    && (path_str.ends_with(EXT_WXS_YAML)
                        || path_str.ends_with(EXT_WXI_YAML)
                        || path_str.ends_with(EXT_WXL_YAML))
                {
                    let xml_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().replace(".yaml", ""))
                        .unwrap_or_default();
                    let xml_out = temp_dir.join(&xml_name);

                    if let Err(e) = yaml_to_wix::convert_file(
                        &path,
                        &xml_out,
                        AttributeStrategy::Inline,
                        &known_attributes,
                    ) {
                        info!("Skipping {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Load from temp directory
        project.load_from_directory(temp_dir)?;
    } else {
        // Load from input directory (original behavior)
        let root_dir = input
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        project.load_from_directory(&root_dir)?;
    }

    // Extract Info
    let (p_name, p_ver, p_man) = project.find_product_info();

    // Use defaults if missing
    let name = p_name.unwrap_or_else(|| "MyApplication".to_string());
    let version = p_ver.unwrap_or_else(|| "1.0.0.0".to_string());
    let manufacturer = p_man.unwrap_or_else(|| "MyCompany".to_string());

    // Check content for CPack heuristic
    let content = fs::read_to_string(input).unwrap_or_default();
    let is_cpack_project = content.contains("CPACK_");

    // Executable
    let executables = project.get_executables();
    let shortcuts = project.get_shortcut_targets();

    let executable = if let Some(exact) = executables
        .iter()
        .find(|e| e.eq_ignore_ascii_case(&format!("{}.exe", name)))
    {
        exact.clone()
    } else if let Some(first) = executables.first() {
        first.clone()
    } else if let Some(sc) = shortcuts.first() {
        sc.clone()
    } else if is_cpack_project {
        "$(var.CPACK_PACKAGE_NAME).exe".to_string()
    } else {
        "App.exe".to_string()
    };

    let version_quad = clean_version(&version);

    // Generate AppxManifest
    let manifest_filename = if is_template {
        "AppxManifest.xml.in"
    } else {
        "AppxManifest.xml"
    };
    let manifest_path = output_dir.join(manifest_filename);

    let app_icon = project.get_app_icon();

    let mut msix = MsixManifest::new();

    msix.start_package()?;
    msix.map_identity(&name, &manufacturer, &version_quad)?;
    msix.map_properties(&name, &manufacturer, app_icon.as_deref())?;
    msix.map_dependencies()?;
    msix.map_resources()?;

    msix.start_applications()?;
    msix.start_application("App", &executable)?;
    msix.map_visual_elements(&name, app_icon.as_deref())?;
    msix.map_extensions(&project)?;
    msix.end_application()?;
    msix.end_applications()?;

    msix.map_capabilities()?;
    msix.end_package()?;

    fs::write(&manifest_path, msix.finish()?)?;
    info!("Generated MSIX manifest at {}", manifest_path.display());

    let assets_dir = output_dir.join("Assets");
    fs::create_dir_all(&assets_dir)?;
    info!("Created Assets directory at {}", assets_dir.display());

    Ok(())
}

struct MsixManifest {
    writer: Writer<Cursor<Vec<u8>>>,
}

impl MsixManifest {
    fn new() -> Self {
        Self {
            writer: Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2),
        }
    }

    fn finish(self) -> Result<Vec<u8>> {
        Ok(self.writer.into_inner().into_inner())
    }

    fn start_package(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
        let mut pkg = BytesStart::new("Package");
        pkg.push_attribute((
            "xmlns",
            "http://schemas.microsoft.com/appx/manifest/foundation/windows10",
        ));
        pkg.push_attribute((
            "xmlns:uap",
            "http://schemas.microsoft.com/appx/manifest/uap/windows10",
        ));
        pkg.push_attribute(("xmlns:rescap", "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"));
        self.writer.write_event(Event::Start(pkg))?;
        Ok(())
    }

    fn end_package(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::End(BytesEnd::new("Package")))?;
        Ok(())
    }

    fn map_identity(&mut self, name: &str, publisher: &str, version: &str) -> Result<()> {
        let publisher_dn = format!("CN={}", publisher);
        // Create package identity name: Publisher.ProductName (no spaces, alphanumeric)
        let safe_name = format!("{}.{}", publisher.replace(' ', ""), name.replace(' ', ""));
        let mut identity = BytesStart::new("Identity");
        identity.push_attribute(("Name", safe_name.as_str()));
        identity.push_attribute(("Publisher", publisher_dn.as_str()));
        identity.push_attribute(("Version", version));
        identity.push_attribute(("ProcessorArchitecture", "x64"));
        self.writer.write_event(Event::Empty(identity))?;
        Ok(())
    }

    fn map_properties(&mut self, name: &str, publisher: &str, icon: Option<&str>) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new("Properties")))?;
        self.write_simple("DisplayName", name)?;
        self.write_simple("PublisherDisplayName", publisher)?;

        let logo = if let Some(_src) = icon {
            // Ideally we copy/convert the icon. For now, point to a standard name.
            // If src is a var like $(var...), we might want to use it?
            // MSIX requires file paths.
            "Assets\\StoreLogo.png"
        } else {
            "Assets\\StoreLogo.png"
        };
        self.write_simple("Logo", logo)?;

        self.writer
            .write_event(Event::End(BytesEnd::new("Properties")))?;
        Ok(())
    }

    fn map_dependencies(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new("Dependencies")))?;
        let mut target = BytesStart::new("TargetDeviceFamily");
        target.push_attribute(("Name", "Windows.Desktop"));
        target.push_attribute(("MinVersion", "10.0.17763.0"));
        target.push_attribute(("MaxVersionTested", "10.0.19041.0"));
        self.writer.write_event(Event::Empty(target))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("Dependencies")))?;
        Ok(())
    }

    fn map_resources(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new("Resources")))?;
        let mut res = BytesStart::new("Resource");
        res.push_attribute(("Language", "x-generate"));
        self.writer.write_event(Event::Empty(res))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("Resources")))?;
        Ok(())
    }

    fn start_applications(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new("Applications")))?;
        Ok(())
    }

    fn end_applications(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::End(BytesEnd::new("Applications")))?;
        Ok(())
    }

    fn start_application(&mut self, id: &str, executable: &str) -> Result<()> {
        let mut app = BytesStart::new("Application");
        app.push_attribute(("Id", id));
        app.push_attribute(("Executable", executable));
        app.push_attribute(("EntryPoint", "Windows.FullTrustApplication"));
        self.writer.write_event(Event::Start(app))?;
        Ok(())
    }

    fn end_application(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::End(BytesEnd::new("Application")))?;
        Ok(())
    }

    fn map_visual_elements(&mut self, name: &str, icon: Option<&str>) -> Result<()> {
        let mut visual = BytesStart::new("uap:VisualElements");
        visual.push_attribute(("DisplayName", name));
        visual.push_attribute(("Description", name));
        visual.push_attribute(("BackgroundColor", "transparent"));

        let logo = if let Some(src) = icon {
            if src.contains('@') || src.contains("$(") {
                src
            } else {
                "Assets\\Square150x150Logo.png"
            }
        } else {
            "Assets\\Square150x150Logo.png"
        };

        visual.push_attribute(("Square150x150Logo", logo));
        visual.push_attribute(("Square44x44Logo", logo)); // Reuse for now
        self.writer.write_event(Event::Empty(visual))?;
        Ok(())
    }

    fn map_extensions(&mut self, project: &WixProject) -> Result<()> {
        let associations = project.get_file_associations();
        if associations.is_empty() {
            return Ok(());
        }

        self.writer
            .write_event(Event::Start(BytesStart::new("Extensions")))?;

        for assoc in associations {
            let mut ext = BytesStart::new("uap:Extension");
            ext.push_attribute(("Category", "windows.fileTypeAssociation"));
            self.writer.write_event(Event::Start(ext))?;

            let mut fta = BytesStart::new("uap:FileTypeAssociation");
            fta.push_attribute(("Name", assoc.name.as_str()));

            // Resolve Logo
            if let Some(icon_id) = &assoc.icon
                && let Some(src) = project.get_file_source(icon_id)
            {
                if src.contains('@') || src.contains("$(") {
                    fta.push_attribute(("Logo", src.as_str()));
                } else {
                    fta.push_attribute(("Logo", "Assets\\FileIcon.png"));
                }
            }

            self.writer.write_event(Event::Start(fta))?;

            if let Some(desc) = &assoc.description {
                self.write_simple("uap:DisplayName", desc)?;
            }

            let supported = BytesStart::new("uap:SupportedFileTypes");
            self.writer.write_event(Event::Start(supported))?;

            for file_ext in assoc.extensions {
                let dot_ext = if file_ext.starts_with('.') {
                    file_ext
                } else {
                    format!(".{}", file_ext)
                };
                self.write_simple("uap:FileType", &dot_ext)?;
            }

            self.writer
                .write_event(Event::End(BytesEnd::new("uap:SupportedFileTypes")))?;
            self.writer
                .write_event(Event::End(BytesEnd::new("uap:FileTypeAssociation")))?;
            self.writer
                .write_event(Event::End(BytesEnd::new("uap:Extension")))?;
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("Extensions")))?;
        Ok(())
    }

    fn map_capabilities(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new("Capabilities")))?;
        let mut cap = BytesStart::new("rescap:Capability");
        cap.push_attribute(("Name", "runFullTrust"));
        self.writer.write_event(Event::Empty(cap))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("Capabilities")))?;
        Ok(())
    }

    fn write_simple(&mut self, name: &str, text: &str) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new(name)))?;
        self.writer.write_event(Event::Text(BytesText::new(text)))?;
        self.writer.write_event(Event::End(BytesEnd::new(name)))?;
        Ok(())
    }
}

fn clean_version(v: &str) -> String {
    // Ensure x.y.z.w format
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() >= 4 {
        format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3])
    } else {
        let mut p = parts.into_iter().map(String::from).collect::<Vec<_>>();
        while p.len() < 4 {
            p.push("0".to_string());
        }
        p.join(".")
    }
}

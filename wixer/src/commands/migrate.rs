use crate::project::WixProject;
use anyhow::Result;
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

    let is_template = input.extension().map_or(false, |e| e == "in");
    
    // Load Project
    let root_dir = crate::utils::find_wixer_root().map(|p| p.parent().unwrap_or(&p).to_path_buf()).unwrap_or_else(|_| input.parent().unwrap_or_else(|| Path::new(".")).to_path_buf());
    
    let mut project = WixProject::new();
    project.load_from_directory(&root_dir)?;

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
    let executable = if let Some(exact) = executables.iter().find(|e| e.eq_ignore_ascii_case(&format!("{}.exe", name))) {
        exact.clone()
    } else if let Some(first) = executables.first() {
        first.clone()
    } else if is_cpack_project {
        "$(var.CPACK_PACKAGE_NAME).exe".to_string()
    } else {
        "App.exe".to_string()
    };

    let version_quad = clean_version(&version);

    // Generate AppxManifest
    let manifest_filename = if is_template { "AppxManifest.xml.in" } else { "AppxManifest.xml" };
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
        self.writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
        let mut pkg = BytesStart::new("Package");
        pkg.push_attribute(("xmlns", "http://schemas.microsoft.com/appx/manifest/foundation/windows10"));
        pkg.push_attribute(("xmlns:uap", "http://schemas.microsoft.com/appx/manifest/uap/windows10"));
        pkg.push_attribute(("xmlns:rescap", "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"));
        self.writer.write_event(Event::Start(pkg))?;
        Ok(())
    }

    fn end_package(&mut self) -> Result<()> {
        self.writer.write_event(Event::End(BytesEnd::new("Package")))?;
        Ok(())
    }

    fn map_identity(&mut self, _name: &str, publisher: &str, version: &str) -> Result<()> {
        let publisher_dn = format!("CN={}", publisher);
        let mut identity = BytesStart::new("Identity");
        identity.push_attribute(("Name", "MyPackage")); // Should use safe name
        identity.push_attribute(("Publisher", publisher_dn.as_str()));
        identity.push_attribute(("Version", version));
        identity.push_attribute(("ProcessorArchitecture", "x64"));
        self.writer.write_event(Event::Empty(identity))?;
        Ok(())
    }

    fn map_properties(&mut self, name: &str, publisher: &str, icon: Option<&str>) -> Result<()> {
        self.writer.write_event(Event::Start(BytesStart::new("Properties")))?;
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
        
        self.writer.write_event(Event::End(BytesEnd::new("Properties")))?;
        Ok(())
    }

    fn map_dependencies(&mut self) -> Result<()> {
        self.writer.write_event(Event::Start(BytesStart::new("Dependencies")))?;
        let mut target = BytesStart::new("TargetDeviceFamily");
        target.push_attribute(("Name", "Windows.Desktop"));
        target.push_attribute(("MinVersion", "10.0.17763.0"));
        target.push_attribute(("MaxVersionTested", "10.0.19041.0"));
        self.writer.write_event(Event::Empty(target))?;
        self.writer.write_event(Event::End(BytesEnd::new("Dependencies")))?;
        Ok(())
    }

    fn map_resources(&mut self) -> Result<()> {
        self.writer.write_event(Event::Start(BytesStart::new("Resources")))?;
        let mut res = BytesStart::new("Resource");
        res.push_attribute(("Language", "x-generate"));
        self.writer.write_event(Event::Empty(res))?;
        self.writer.write_event(Event::End(BytesEnd::new("Resources")))?;
        Ok(())
    }

    fn start_applications(&mut self) -> Result<()> {
        self.writer.write_event(Event::Start(BytesStart::new("Applications")))?;
        Ok(())
    }

    fn end_applications(&mut self) -> Result<()> {
        self.writer.write_event(Event::End(BytesEnd::new("Applications")))?;
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
        self.writer.write_event(Event::End(BytesEnd::new("Application")))?;
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

        self.writer.write_event(Event::Start(BytesStart::new("Extensions")))?;

        for assoc in associations {
            let mut ext = BytesStart::new("uap:Extension");
            ext.push_attribute(("Category", "windows.fileTypeAssociation"));
            self.writer.write_event(Event::Start(ext))?;

            let mut fta = BytesStart::new("uap:FileTypeAssociation");
            fta.push_attribute(("Name", assoc.name.as_str()));
            
            // Resolve Logo
            if let Some(icon_id) = &assoc.icon {
                if let Some(src) = project.get_file_source(icon_id) {
                    if src.contains('@') || src.contains("$(") {
                        fta.push_attribute(("Logo", src.as_str()));
                    } else {
                        fta.push_attribute(("Logo", "Assets\\FileIcon.png"));
                    }
                }
            }

            self.writer.write_event(Event::Start(fta))?;

            if let Some(desc) = &assoc.description {
                self.write_simple("uap:DisplayName", desc)?;
            }

            let supported = BytesStart::new("uap:SupportedFileTypes");
            self.writer.write_event(Event::Start(supported))?;

            for file_ext in assoc.extensions {
                let dot_ext = if file_ext.starts_with('.') { file_ext } else { format!(".{}", file_ext) };
                self.write_simple("uap:FileType", &dot_ext)?;
            }

            self.writer.write_event(Event::End(BytesEnd::new("uap:SupportedFileTypes")))?;
            self.writer.write_event(Event::End(BytesEnd::new("uap:FileTypeAssociation")))?;
            self.writer.write_event(Event::End(BytesEnd::new("uap:Extension")))?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("Extensions")))?;
        Ok(())
    }

    fn map_capabilities(&mut self) -> Result<()> {
        self.writer.write_event(Event::Start(BytesStart::new("Capabilities")))?;
        let mut cap = BytesStart::new("rescap:Capability");
        cap.push_attribute(("Name", "runFullTrust"));
        self.writer.write_event(Event::Empty(cap))?;
        self.writer.write_event(Event::End(BytesEnd::new("Capabilities")))?;
        Ok(())
    }

    fn write_simple(&mut self, name: &str, text: &str) -> Result<()> {
        self.writer.write_event(Event::Start(BytesStart::new(name)))?;
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

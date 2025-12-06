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

    // Pick best executable
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

    // Clean version for MSIX (Quad format)
    let version_quad = clean_version(&version);

    // Generate AppxManifest.xml
    let manifest_filename = if is_template { "AppxManifest.xml.in" } else { "AppxManifest.xml" };
    let manifest_path = output_dir.join(manifest_filename);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    // <Package>
    let mut pkg = BytesStart::new("Package");
    pkg.push_attribute((
        "xmlns",
        "http://schemas.microsoft.com/appx/manifest/foundation/windows10",
    ));
    pkg.push_attribute((
        "xmlns:uap",
        "http://schemas.microsoft.com/appx/manifest/uap/windows10",
    ));
    pkg.push_attribute((
        "xmlns:rescap",
        "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities",
    ));
    writer.write_event(Event::Start(pkg))?;

    // <Identity>
    let publisher_dn = format!("CN={}", manufacturer);
    let mut identity = BytesStart::new("Identity");
    identity.push_attribute(("Name", "MyPackage"));
    identity.push_attribute(("Publisher", publisher_dn.as_str()));
    identity.push_attribute(("Version", version_quad.as_str()));
    identity.push_attribute(("ProcessorArchitecture", "x64"));
    writer.write_event(Event::Empty(identity))?;

    // <Properties>
    let props = BytesStart::new("Properties");
    writer.write_event(Event::Start(props))?;

    write_simple_element(&mut writer, "DisplayName", &name)?;
    write_simple_element(&mut writer, "PublisherDisplayName", &manufacturer)?;
    write_simple_element(&mut writer, "Logo", "Assets\\StoreLogo.png")?;

    writer.write_event(Event::End(BytesEnd::new("Properties")))?;

    // <Dependencies>
    let deps = BytesStart::new("Dependencies");
    writer.write_event(Event::Start(deps))?;

    let mut target_dev = BytesStart::new("TargetDeviceFamily");
    target_dev.push_attribute(("Name", "Windows.Desktop"));
    target_dev.push_attribute(("MinVersion", "10.0.17763.0"));
    target_dev.push_attribute(("MaxVersionTested", "10.0.19041.0"));
    writer.write_event(Event::Empty(target_dev))?;

    writer.write_event(Event::End(BytesEnd::new("Dependencies")))?;

    // <Resources>
    let resources = BytesStart::new("Resources");
    writer.write_event(Event::Start(resources))?;

    let mut res = BytesStart::new("Resource");
    res.push_attribute(("Language", "x-generate"));
    writer.write_event(Event::Empty(res))?;

    writer.write_event(Event::End(BytesEnd::new("Resources")))?;

    // <Applications>
    let apps = BytesStart::new("Applications");
    writer.write_event(Event::Start(apps))?;

    let mut app = BytesStart::new("Application");
    app.push_attribute(("Id", "App"));
    app.push_attribute(("Executable", executable.as_str()));
    app.push_attribute(("EntryPoint", "Windows.FullTrustApplication"));
    writer.write_event(Event::Start(app))?;

    // <uap:VisualElements>
    let mut visual = BytesStart::new("uap:VisualElements");
    visual.push_attribute(("DisplayName", name.as_str()));
    visual.push_attribute(("Description", name.as_str()));
    visual.push_attribute(("BackgroundColor", "transparent"));
    visual.push_attribute(("Square150x150Logo", "Assets\\Square150x150Logo.png"));
    visual.push_attribute(("Square44x44Logo", "Assets\\Square44x44Logo.png"));
    writer.write_event(Event::Empty(visual))?;

    // Extensions (File Associations)
    let associations = project.get_file_associations();
    if !associations.is_empty() {
        let exts = BytesStart::new("Extensions");
        writer.write_event(Event::Start(exts))?;

        for assoc in associations {
            let mut ext = BytesStart::new("uap:Extension");
            ext.push_attribute(("Category", "windows.fileTypeAssociation"));
            writer.write_event(Event::Start(ext))?;

            let mut fta = BytesStart::new("uap:FileTypeAssociation");
            fta.push_attribute(("Name", assoc.name.as_str()));
            writer.write_event(Event::Start(fta))?;

            if let Some(desc) = &assoc.description {
                write_simple_element(&mut writer, "uap:DisplayName", desc)?;
            }
            
            // TODO: Logo (assoc.icon) mapping?

            let supported = BytesStart::new("uap:SupportedFileTypes");
            writer.write_event(Event::Start(supported))?;

            for file_ext in assoc.extensions {
                let dot_ext = if file_ext.starts_with('.') { file_ext } else { format!(".{}", file_ext) };
                write_simple_element(&mut writer, "uap:FileType", &dot_ext)?;
            }

            writer.write_event(Event::End(BytesEnd::new("uap:SupportedFileTypes")))?;
            writer.write_event(Event::End(BytesEnd::new("uap:FileTypeAssociation")))?;
            writer.write_event(Event::End(BytesEnd::new("uap:Extension")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("Extensions")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("Application")))?;
    writer.write_event(Event::End(BytesEnd::new("Applications")))?;

    // <Capabilities>
    let caps = BytesStart::new("Capabilities");
    writer.write_event(Event::Start(caps))?;

    let mut cap = BytesStart::new("rescap:Capability");
    cap.push_attribute(("Name", "runFullTrust"));
    writer.write_event(Event::Empty(cap))?;

    writer.write_event(Event::End(BytesEnd::new("Capabilities")))?;

    writer.write_event(Event::End(BytesEnd::new("Package")))?;

    let result = writer.into_inner().into_inner();
    fs::write(&manifest_path, result)?;

    info!("Generated MSIX manifest at {}", manifest_path.display());

    // Create basic assets folder structure
    let assets_dir = output_dir.join("Assets");
    fs::create_dir_all(&assets_dir)?;
    info!("Created Assets directory at {}", assets_dir.display());

    Ok(())
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

fn write_simple_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    text: &str,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(text)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

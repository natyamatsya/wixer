use anyhow::{Context, Result};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use regex::Regex;
use roxmltree::{Document, Node};
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

    let content = fs::read_to_string(input).context("Failed to read input file")?;

    // Pre-process to handle CPack variables
    let re = Regex::new(r"(@[A-Z_]+@)").unwrap();
    let processed_content = re.replace_all(&content, |caps: &regex::Captures| {
        let var = &caps[0];
        if var == "@CPACK_WIX_CUSTOM_XMLNS_EXPANDED@" {
             // Inject dummy namespaces to satisfy parser
             " xmlns:ui=\"http://dummy\" xmlns:util=\"http://dummy\" CPACK_WIX_CUSTOM_XMLNS_EXPANDED=\"1\"".to_string()
        } else {
            var.to_string()
        }
    });

    let doc = Document::parse(&processed_content).context("Failed to parse WiX XML")?;
    
    // Extract Product/Package Info
    let product = find_product(&doc).context("Could not find Product or Package element")?;

    let name = product.attribute("Name").unwrap_or("MyApplication");
    let version = product.attribute("Version").unwrap_or("1.0.0.0");
    let manufacturer = product.attribute("Manufacturer").unwrap_or("MyCompany");
    let _id = product.attribute("Id").unwrap_or("auto-generated-id");

    // Clean version for MSIX (Quad format)
    let version_quad = clean_version(version);

    // Extract Executable (Entry Point)
    let executable = find_executable(&doc).unwrap_or_else(|| "App.exe".to_string());

    // Generate AppxManifest.xml
    let manifest_path = output_dir.join("AppxManifest.xml");
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

    write_simple_element(&mut writer, "DisplayName", name)?;
    write_simple_element(&mut writer, "PublisherDisplayName", manufacturer)?;
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
    visual.push_attribute(("DisplayName", name));
    visual.push_attribute(("Description", name));
    visual.push_attribute(("BackgroundColor", "transparent"));
    visual.push_attribute(("Square150x150Logo", "Assets\\Square150x150Logo.png"));
    visual.push_attribute(("Square44x44Logo", "Assets\\Square44x44Logo.png"));
    writer.write_event(Event::Empty(visual))?;

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

fn find_product<'a>(doc: &'a Document) -> Option<Node<'a, 'a>> {
    doc.descendants()
        .find(|n| n.tag_name().name() == "Product" || n.tag_name().name() == "Package")
}

fn find_executable(doc: &Document) -> Option<String> {
    // Look for File element with .exe extension
    doc.descendants()
        .find(|n| {
            n.tag_name().name() == "File"
                && n.attribute("Source")
                    .is_some_and(|s| s.to_lowercase().ends_with(".exe"))
        })
        .and_then(|n| {
            n.attribute("Source").map(|s| {
                // Extract filename from source path
                Path::new(s)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
        })
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

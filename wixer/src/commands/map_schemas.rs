use crate::schema_mapping::SchemaMapper;
use anyhow::Result;

pub fn run() -> Result<()> {
    let mapper = SchemaMapper::new()?;
    let matches = mapper.get_mapping();

    println!("Mapping Analysis:");
    println!(
        "{:<30} | {:<40} | {:<20}",
        "WiX Element", "Potential MSIX Match", "MSIX Source"
    );
    println!("{:-<30}-+-{:-<40}-+-{:-<20}", "", "", "");

    for (wix_name, match_name, source) in &matches {
        println!("{:<30} | {:<40} | {:<20}", wix_name, match_name, source);
    }

    println!(
        "\nFound {} potential matches.",
        matches.len()
    );

    Ok(())
}

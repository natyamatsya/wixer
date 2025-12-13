use anyhow::Result;
use std::fs;
use std::path::Path;

use super::discover::Example;

const INDEX_PREAMBLE: &str = r#"# Wixer Example Gallery

This gallery contains WiX installer examples converted for use with wixer.

## Structure

- **wix-v4/** — WiX v4 XML files (upgraded from v3)
- **wixer-yaml/** — Curated YAML gallery (tracked in git)
- **msi/** — Built MSI packages

## Curated YAML Gallery

The curated YAML gallery lives under `examples/wixer-yaml/` in the repository.

### Known Pitfalls

The `known-pitfalls` example is intentionally invalid and is used to demonstrate common WiX mistakes.

When building the gallery, `known-pitfalls` is configured with `expect_build_failure: true`.
This means a failing build is treated as success and shown as `✓ (expected failure)`.

## Submodule Examples (upgraded from v3)

| Example | Description |
|---------|-------------|
"#;

const INDEX_USAGE: &str = r#"
## Usage

```bash
# Build an example with wixer
wixer build wixer-yaml/00_HelloWorldInstaller/Product.wxs.yaml

# Or use the v4 XML directly with wix.exe
wix build wix-v4/00_HelloWorldInstaller/Product.wxs
```

## Rebuilding the Gallery

```bash
# From the wixer directory:
cargo run --bin wixer-gallery -- --clean
```

## Source

Examples are derived from [WiXInstallerExamples](https://github.com/kurtanr/WiXInstallerExamples) by kurtanr.
"#;

pub(super) fn generate_index(output: &Path, examples: &[Example]) -> Result<()> {
    let mut content = String::new();

    content.push_str(INDEX_PREAMBLE);

    for example in examples {
        // Try to extract description from README
        let readme_path = example.path.join("README.md");
        let description = if readme_path.exists() {
            extract_first_paragraph(&readme_path).unwrap_or_default()
        } else {
            String::new()
        };

        content.push_str(&format!(
            "| [{}](wix-v4/{}) | {} |\n",
            example.name,
            example.name,
            description.chars().take(80).collect::<String>()
        ));
    }

    content.push_str(INDEX_USAGE);

    fs::write(output.join("README.md"), content)?;

    Ok(())
}

fn extract_first_paragraph(readme: &Path) -> Result<String> {
    let content = fs::read_to_string(readme)?;

    // Skip title and find first paragraph
    let mut in_paragraph = false;
    let mut paragraph = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip headers and empty lines until we find content
        if trimmed.starts_with('#') || trimmed.is_empty() {
            if in_paragraph {
                break;
            }
            continue;
        }

        // Skip badges
        if trimmed.starts_with("[![") || trimmed.starts_with("<br") {
            continue;
        }

        in_paragraph = true;
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }

    Ok(paragraph)
}

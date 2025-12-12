# Embedded Assets

## Why embedded assets exist

Wixer is intended to work in CI and offline environments without requiring a pre-populated schema cache.

To support that, Wixer embeds the following assets into the binary at compile time:

- WiX v4 XSD schemas
- MSIX XSD schemas
- WiX extension metadata JSON (custom actions, sequences, properties)
- A knowledge base (`wixer-knowledge-base.json`)
- Pre-generated derived files:
  - JSON Schemas for validation (`generated/json-schema/*.schema.json`)
  - `generated/describe.json` (unified describe database)
  - `generated/known_attributes.json`
  - `generated/versions.json`

## Runtime behavior

- Normal CLI usage does **not** require any prior initialization.
- `wixer init` is retained for backwards compatibility and to display embedded asset information, but it does not download anything.

### Commands that use embedded assets

- `validate`
  - Uses embedded JSON Schemas by default.
  - Can optionally validate against a user-provided schema path via `--schema`.

- `describe`
  - Loads the embedded `generated/describe.json`.

- `lsp`
  - Loads embedded JSON Schemas for completions/hover/diagnostics.

- `doc`
  - Generates documentation from embedded WiX XSD schemas.

## How assets are built

Embedded assets are packaged into a store-only zip file:

- `wixer/assets/assets.zip`

### `build.rs`

The crate build script (`wixer/build.rs`) assembles `assets.zip` from the `wixer/assets/` directory.

### `prebuild-assets` (development utility)

The `prebuild-assets` binary is used during development to (re)create the asset inputs under `wixer/assets/`:

- Downloads WiX and MSIX XSD schemas if missing
- Generates JSON Schemas from XSD
- Generates `describe.json`, `known_attributes.json`, and `versions.json`

This tool is not intended to be shipped to end users.

## What is still written to disk

Even with embedded assets, Wixer may still write project-local intermediates when running `wix build` through Wixer:

- `.wixer/intermediates/`

This is separate from the old global on-disk schema cache approach.

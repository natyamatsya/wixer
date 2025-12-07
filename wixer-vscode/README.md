# Wixer VS Code Extension

Language support for WiX YAML files (`.wxs.yaml`, `.wxi.yaml`, `.wxl.yaml`) using the wixer Language Server.

## Features

- **Syntax Highlighting** — WiX-specific YAML highlighting
- **LSP Integration** — Completions, hover, and diagnostics via wixer LSP
- **File Association** — Automatic language detection for `.wxs.yaml` files

## Requirements

- [wixer](https://github.com/natyamatsya/wixer) must be installed and available in PATH, or configure `wixer.executablePath`

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `wixer.executablePath` | `wixer` | Path to the wixer executable |
| `wixer.trace.server` | `off` | LSP trace level: `off`, `messages`, `verbose` |

## Development

```bash
cd wixer-vscode
npm install
npm run compile
```

Press F5 in VS Code to launch the extension in debug mode.

## Building

```bash
npm run vscode:prepublish
vsce package
```

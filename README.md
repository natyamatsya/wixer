# wixer

## Configuration

Wixer reads configuration from the following files in order of precedence:

1. `wixer.yaml` found by walking up the directory tree from the current working directory.
2. `wixer.yaml` in the global configuration directory.

The global configuration directory is determined by the operating system:
- **Windows**: `%APPDATA%\natyamatsya\wixer\config`
- **Linux**: `~/.config/wixer`
- **macOS**: `~/Library/Application Support/com.natyamatsya.wixer`

## Cache Storage

Wixer stores downloaded schemas (WiX, MSIX) and the generated description database (`describe.json`) in a global cache directory. This avoids redownloading schemas for every project.

The cache directory is located at:
- **Windows**: `%LOCALAPPDATA%\natyamatsya\wixer\cache`
- **Linux**: `~/.cache/wixer`
- **macOS**: `~/Library/Caches/com.natyamatsya.wixer`

To clear or refresh the cache, run:
```bash
wixer init --fresh
```

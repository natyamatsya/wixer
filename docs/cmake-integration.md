# CMake Integration

Wixer provides CMake modules for seamless integration with CMake/CPack workflows. This enables transparent YAML support in existing WiX-based build systems.

## Overview

The integration consists of three main components:

1. **wixer_impersonate** — Intercepts `wix.exe` calls so CPack uses wixer transparently
2. **wixer_add_yaml_sources** — Exports YAML to XML as a build dependency
3. **wixer_msix_package** — Auto-generates AppxManifest from YAML for MSIX packaging

## Setup

Add the wixer cmake directory to your module path:

```cmake
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")
include(WixerImpersonate)
```

## wixer_impersonate

Sets up wixer to intercept WiX toolset calls. When CPack invokes `wix.exe`, it actually runs wixer, which handles YAML conversion and provides enhanced debugging.

```cmake
wixer_impersonate(
    WIXER_EXE <path>           # Path to wixer executable
    [IMPERSONATE_DIR <path>]   # Directory for impersonation files (default: ${CMAKE_BINARY_DIR}/_wixer_impersonate)
    [VERBOSE]                  # Enable verbose output
    [INSPECT_PDB]              # Enable PDB inspection after build
    [INSPECT_SYMBOLS <list>]   # Symbols to inspect in PDB
    [LOG_FILE <path>]          # Log file path (default: ${CMAKE_BINARY_DIR}/wixer.log)
)
```

### Example

```cmake
wixer_impersonate(
    WIXER_EXE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe"
    VERBOSE
    INSPECT_PDB
)
```

### Generated Files

The function generates helper scripts in the build directory:

- `wixer_env.ps1` — PowerShell script to set environment variables
- `wixer_env.sh` — Bash script for Linux/macOS
- `wixer_env.cmd` — CMD script for Windows
- `wixer_pack.ps1` — Wrapper that runs CPack and displays the wixer log

### Usage with CPack

**Important:** CMake's WIX generator caches the WiX path at startup. You must set environment variables *before* running cpack:

```powershell
# PowerShell
. ./build/wixer_env.ps1
cpack -G WIX

# Or use the wrapper script
./build/wixer_pack.ps1
```

---

## wixer_add_yaml_sources

Adds a custom target to export YAML WiX sources to XML. The export runs automatically when sources change.

```cmake
wixer_add_yaml_sources(<target_name> <yaml_source_dir> <output_dir>)
```

### Parameters

- `target_name` — Name for the custom target (e.g., `wixer_yaml_export`)
- `yaml_source_dir` — Directory containing `.wxs.yaml`, `.wxi.yaml`, `.wxl.yaml` files
- `output_dir` — Directory where `.wxs`, `.wxi`, `.wxl` files will be generated

### Variables Set

- `WIXER_YAML_OUTPUT_DIR` — The output directory
- `WIXER_YAML_OUTPUT_FILES` — List of generated XML files

### Example

```cmake
set(WIXER_EXECUTABLE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe")
wixer_add_yaml_sources(wixer_yaml_export
    "${CMAKE_SOURCE_DIR}/wix"
    "${CMAKE_BINARY_DIR}/wix"
)

# Build the export target
# ninja wixer_yaml_export

# Use exported files with CPack
set(CPACK_WIX_EXTRA_SOURCES ${WIXER_YAML_OUTPUT_FILES})
```

---

## wixer_msix_package

Configures CPack External generator to create MSIX packages using wixer's migrate command. This auto-generates `AppxManifest.xml` from WiX YAML sources.

```cmake
wixer_msix_package(<yaml_source>
    [APPX_NAME <name>]         # Package identity name
    [APPX_VERSION <version>]   # Package version (default: ${PROJECT_VERSION}.0)
    [ASSETS_DIR <path>]        # Logo assets directory (default: ${CMAKE_SOURCE_DIR}/assets)
)
```

### Variables Set

- `WIXER_MSIX_PACKAGE_SCRIPT` — Path to the generated CPack script

### Example

```cmake
wixer_msix_package("${CMAKE_SOURCE_DIR}/myproject.wxs.yaml"
    APPX_NAME "MyCompany.MyApp"
    APPX_VERSION "1.0.0.0"
)
```

---

## Complete Example

```cmake
cmake_minimum_required(VERSION 3.30)
project(MyApp VERSION 1.0.0)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")

install(PROGRAMS target/release/myapp.exe DESTINATION bin)
install(DIRECTORY assets DESTINATION .)

if(WIN32)
    include(WixerImpersonate)
    
    set(WIXER_EXECUTABLE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe")
    
    # Set up impersonation for CPack WIX generator
    wixer_impersonate(
        WIXER_EXE "${WIXER_EXECUTABLE}"
        VERBOSE
    )
    
    # Export YAML sources to XML
    wixer_add_yaml_sources(yaml_export
        "${CMAKE_SOURCE_DIR}/wix"
        "${CMAKE_BINARY_DIR}/wix"
    )
    
    # Configure MSIX packaging
    wixer_msix_package("${CMAKE_SOURCE_DIR}/wix/myapp.wxs.yaml"
        APPX_NAME "MyCompany.MyApp"
    )
endif()

# CPack configuration
set(CPACK_PACKAGE_NAME "MyApp")
set(CPACK_PACKAGE_VENDOR "MyCompany")
set(CPACK_WIX_VERSION "4")
set(CPACK_WIX_UPGRADE_GUID "your-guid-here")

if(WIN32)
    set(CPACK_EXTERNAL_ENABLE_PACKAGING ON)
    set(CPACK_EXTERNAL_ENABLE_STAGING ON)
    
    # Choose MSIX strategy: "legacy" or "wixer"
    if(WIXER_MSIX_STRATEGY STREQUAL "wixer")
        set(CPACK_EXTERNAL_PACKAGE_SCRIPT "${WIXER_MSIX_PACKAGE_SCRIPT}")
    else()
        set(CPACK_EXTERNAL_PACKAGE_SCRIPT "${CMAKE_BINARY_DIR}/MakeAppX.cmake")
    endif()
    
    set(CPACK_GENERATOR "WIX;External")
endif()

include(CPack)
```

---

## CMake Presets

Wixer includes preset configurations for common workflows:

### Configure Presets

| Preset | Description |
|--------|-------------|
| `wixer-rel` | Release build with wixer impersonation (legacy MSIX) |
| `wixer-rel-auto` | Release build with auto-generated MSIX from YAML |

### Package Presets

| Preset | Description |
|--------|-------------|
| `wixer-rel-pkg` | Build MSI with wixer impersonation |
| `wixer-rel-msix` | Build MSIX (legacy AppxManifest) |
| `wixer-auto-msix` | Build MSIX (auto-generated from YAML) |

### Build Presets

| Preset | Description |
|--------|-------------|
| `wixer-yaml-export` | Export YAML WiX sources to XML |
| `vscode-extension` | Build VS Code extension (.vsix) |

### Workflow Presets

| Preset | Description |
|--------|-------------|
| `wixer-rel-pkg` | Full pipeline: configure → build VS Code extension → export YAML → build MSI |
| `wixer-auto-msix` | Configure and build MSIX with auto AppxManifest |

### Usage

```bash
# Full MSI build workflow (includes VS Code extension)
cmake --workflow --preset wixer-rel-pkg

# Auto-generated MSIX from YAML
cmake --workflow --preset wixer-auto-msix

# Or step by step
cmake --preset wixer-rel
cmake --build --preset vscode-extension    # Build VS Code extension
cmake --build --preset wixer-yaml-export   # Export YAML to XML
cpack --preset wixer-rel-pkg               # Build MSI
```

---

## Troubleshooting

### CPack uses real wix.exe instead of wixer

CMake's WIX generator checks PATH before the WIX environment variable. The helper scripts prepend wixer's bin directory to PATH:

```powershell
. ./build/wixer_env.ps1
cpack -G WIX
```

### No log output

Ensure `wixer.yaml` has logging enabled:

```yaml
logging:
  file: "wixer.log"
```

Or use `wixer_impersonate` with logging configured (enabled by default).

### YAML changes not reflected

Run the export target to regenerate XML:

```bash
cmake --build --preset wixer-yaml-export
```

# 08 - C++ Custom Action Example

A modern C++23 custom action example for WiX v4+ installers.

This is a modernized replacement for the legacy WiX SDK-based example in the submodule.

## Key Changes from Legacy

| Legacy | Modern |
|--------|--------|
| Visual Studio .vcxproj | CMake |
| WiX SDK (wcautil.lib) | Direct MSI API |
| C++14 | C++23 |
| Win32 only | x64/x86 via presets |

## Building

```bash
cd MyCppCustomAction

# Configure and build (requires CMake 3.25+ and Visual Studio 2022 or later;
# the presets use the newest installed Visual Studio, no Developer shell needed)
cmake --preset default
cmake --build --preset default

# Or for x86:
cmake --preset x86
cmake --build --preset x86
```

The DLL will be in `MyCppCustomAction/build/MyCppCustomAction.dll`.

Then build the installer:
```bash
cd ..
wixer build Product.wxs.yaml
```

## Custom Actions

| Action | Type | Description |
|--------|------|-------------|
| `MyImmediateCustomAction` | Immediate | Reads/writes properties via MSI API |
| `MyDeferredCustomAction` | Deferred | Runs elevated, uses CustomActionData |
| `MyRollbackCustomAction` | Rollback | Undoes changes on failure |

## Modern C++23 Features Used

- `std::expected` for error handling
- `std::format` for string formatting
- `std::wstring_view` for string parameters
- No WiX SDK dependency (uses MSI API directly)

## Debugging

1. Build with Debug preset: `cmake --preset debug && cmake --build --preset debug`
2. Run installer with logging: `msiexec /i MyInstaller.msi /l*v install.log`
3. Attach debugger to `msiexec.exe` process

## API Reference

The custom action uses the Windows Installer API directly:

- `MsiGetPropertyW` - Read properties
- `MsiSetPropertyW` - Set properties  
- `MsiProcessMessage` - Log messages
- `MsiCreateRecord` - Create log records

No WiX SDK (wcautil) dependency - just `msi.lib` from the Windows SDK.

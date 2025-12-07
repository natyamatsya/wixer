# Modern C# Custom Action Example

A modern C# custom action targeting **.NET 6** and **.NET 8** for use with WiX v6+ installers.

## Target Frameworks

| Framework | WiX Version | Status |
|-----------|-------------|--------|
| `net6.0-windows` | WiX v6.x (stable) | ✅ Supported |
| `net8.0-windows` | WiX mainline (v7) | ✅ Supported |


## Building

```bash
# Build the C# custom action DLL
cd MyCSharpCustomAction
dotnet publish -c Release -r win-x64 -f net6.0-windows

# Then build the installer
cd ..
wixer build Product.wxs.yaml
```

## Custom Actions Included

| Action | Type | Description |
|--------|------|-------------|
| `SampleImmediateAction` | Immediate | Reads/writes properties, validates input |
| `SampleDeferredAction` | Deferred | Runs elevated, uses CustomActionData |
| `SampleRollbackAction` | Rollback | Undoes deferred action on failure |

## Using in WiX

### WiX v4/v6 Syntax

```xml
<!-- Reference the CA DLL -->
<Binary Id="CustomActionBinary" 
        SourceFile="path\to\MyCSharpCustomAction.dll" />

<!-- Immediate action -->
<CustomAction Id="CA_Immediate" 
              BinaryRef="CustomActionBinary" 
              DllEntry="SampleImmediateAction" 
              Execute="immediate" 
              Return="check" />

<!-- Set up data for deferred action -->
<SetProperty Id="CA_Deferred" 
             Before="CA_Deferred" 
             Sequence="execute"
             Value="InstallDir=[INSTALLFOLDER];MyProperty=[MY_PROPERTY]" />

<!-- Deferred action (runs elevated) -->
<CustomAction Id="CA_Deferred" 
              BinaryRef="CustomActionBinary" 
              DllEntry="SampleDeferredAction" 
              Execute="deferred" 
              Return="check" 
              Impersonate="no" />

<!-- Rollback action -->
<CustomAction Id="CA_Rollback" 
              BinaryRef="CustomActionBinary" 
              DllEntry="SampleRollbackAction" 
              Execute="rollback" 
              Return="check" 
              Impersonate="no" />

<!-- Schedule the actions -->
<InstallExecuteSequence>
  <Custom Action="CA_Immediate" Before="CA_Deferred">NOT Installed</Custom>
  <Custom Action="CA_Rollback" Before="CA_Deferred">NOT Installed</Custom>
  <Custom Action="CA_Deferred" Before="InstallFinalize">NOT Installed</Custom>
</InstallExecuteSequence>
```

## Key Differences from Legacy (.NET Framework) Custom Actions

1. **Self-contained deployment**: The DLL includes the .NET runtime, so target machines don't need .NET installed.

2. **Modern C# features**: Nullable reference types, file-scoped namespaces, pattern matching, etc.

3. **No MakeSfxCA**: Traditional WiX used `MakeSfxCA.exe` to create self-extracting CA DLLs. With modern .NET, we publish self-contained instead.

4. **Larger DLL size**: Self-contained deployment increases DLL size (~15-30 MB), but eliminates runtime dependencies.

## API Reference

The `WixToolset.Dtf.WindowsInstaller` package provides:

- `Session` - Access to the Windows Installer session
- `CustomActionAttribute` - Marks methods as custom actions
- `ActionResult` - Return values (Success, Failure, NotExecuted, etc.)
- `CustomActionData` - Key-value data for deferred actions
- `Record`, `View`, `Database` - MSI database access

## Debugging

1. Add `session.Log()` calls throughout your code
2. Run the installer with logging: `msiexec /i MyInstaller.msi /l*v install.log`
3. Check `install.log` for your custom action output

For interactive debugging, you can add:
```csharp
#if DEBUG
System.Diagnostics.Debugger.Launch();
#endif
```

## Migration from WiX v3 DTF

| WiX v3 | WiX v4+ |
|--------|---------|
| `Microsoft.Deployment.WindowsInstaller` | `WixToolset.Dtf.WindowsInstaller` |
| `BinaryKey` attribute | `BinaryRef` attribute |
| `FileKey` attribute | `FileRef` attribute |
| .NET Framework 4.x | .NET 6/8 (self-contained) |

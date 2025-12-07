/// @file CustomAction.cpp
/// @brief Modern C++23 WiX Custom Action Example
///
/// This is a modernized version of the WiX C++ custom action example.
/// It demonstrates immediate and deferred custom actions without
/// depending on the WiX SDK (wcautil.lib).

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <msiquery.h>
#include <string>
#include <format>
#include <expected>

namespace {

/// Log a message to the MSI log
void LogMessage(MSIHANDLE hInstall, std::wstring_view message) {
    PMSIHANDLE hRecord = MsiCreateRecord(1);
    if (hRecord) {
        MsiRecordSetStringW(hRecord, 0, L"[1]");
        MsiRecordSetStringW(hRecord, 1, message.data());
        MsiProcessMessage(hInstall, INSTALLMESSAGE_INFO, hRecord);
    }
}

/// Get a property value from the MSI session
[[nodiscard]] std::expected<std::wstring, UINT> GetProperty(MSIHANDLE hInstall, std::wstring_view name) {
    DWORD size = 0;
    UINT result = MsiGetPropertyW(hInstall, name.data(), L"", &size);
    
    if (result == ERROR_MORE_DATA) {
        std::wstring value(size, L'\0');
        ++size; // Include null terminator
        result = MsiGetPropertyW(hInstall, name.data(), value.data(), &size);
        if (result == ERROR_SUCCESS) {
            value.resize(size); // Trim to actual size
            return value;
        }
    }
    
    if (result == ERROR_SUCCESS) {
        return std::wstring{};
    }
    
    return std::unexpected(result);
}

} // namespace

/// @brief Immediate custom action - runs with full session access
/// 
/// Can read/write properties directly via MsiGetProperty/MsiSetProperty.
/// Runs in the context of the installing user.
extern "C" __declspec(dllexport) UINT __stdcall MyImmediateCustomAction(MSIHANDLE hInstall) {
    LogMessage(hInstall, L"MyImmediateCustomAction: Starting");
    
    // Read a property
    auto myProperty = GetProperty(hInstall, L"MY_PROPERTY");
    if (myProperty) {
        auto msg = std::format(L"MyImmediateCustomAction: MY_PROPERTY = '{}'", *myProperty);
        LogMessage(hInstall, msg);
    } else {
        LogMessage(hInstall, L"MyImmediateCustomAction: Failed to read MY_PROPERTY");
        return ERROR_INSTALL_FAILURE;
    }
    
    // Set a property for later use
    MsiSetPropertyW(hInstall, L"CA_RESULT", L"ImmediateActionCompleted");
    
    LogMessage(hInstall, L"MyImmediateCustomAction: Completed successfully");
    return ERROR_SUCCESS;
}

/// @brief Deferred custom action - runs elevated during install
/// 
/// Cannot access properties directly. Must use CustomActionData
/// which is set by a preceding immediate action (typically SetProperty).
extern "C" __declspec(dllexport) UINT __stdcall MyDeferredCustomAction(MSIHANDLE hInstall) {
    LogMessage(hInstall, L"MyDeferredCustomAction: Starting");
    
    // Deferred actions can only read CustomActionData
    auto customActionData = GetProperty(hInstall, L"CustomActionData");
    if (customActionData) {
        auto msg = std::format(L"MyDeferredCustomAction: CustomActionData = '{}'", *customActionData);
        LogMessage(hInstall, msg);
        
        // Parse CustomActionData (format: "Key1=Value1;Key2=Value2")
        // In a real CA, you'd parse this to extract individual values
    } else {
        LogMessage(hInstall, L"MyDeferredCustomAction: Failed to read CustomActionData");
        return ERROR_INSTALL_FAILURE;
    }
    
    LogMessage(hInstall, L"MyDeferredCustomAction: Completed successfully");
    return ERROR_SUCCESS;
}

/// @brief Rollback custom action - runs if installation fails
/// 
/// Used to undo changes made by a deferred action.
extern "C" __declspec(dllexport) UINT __stdcall MyRollbackCustomAction(MSIHANDLE hInstall) {
    LogMessage(hInstall, L"MyRollbackCustomAction: Starting rollback");
    
    auto customActionData = GetProperty(hInstall, L"CustomActionData");
    if (customActionData) {
        auto msg = std::format(L"MyRollbackCustomAction: Rolling back with data = '{}'", *customActionData);
        LogMessage(hInstall, msg);
        
        // Undo whatever the deferred action did
    }
    
    LogMessage(hInstall, L"MyRollbackCustomAction: Rollback completed");
    return ERROR_SUCCESS;
}

/// DLL entry point
BOOL WINAPI DllMain(HINSTANCE hInstDLL, DWORD fdwReason, LPVOID lpvReserved) {
    switch (fdwReason) {
    case DLL_PROCESS_ATTACH:
        DisableThreadLibraryCalls(hInstDLL);
        break;
    case DLL_PROCESS_DETACH:
        break;
    }
    return TRUE;
}

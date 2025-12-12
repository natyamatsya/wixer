cmake_minimum_required(VERSION 3.20)
project(wixer VERSION 0.1.0 LANGUAGES CXX)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")

# Build wixer using cargo
add_custom_target(build_wixer ALL
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    COMMENT "Building wixer with cargo"
)

# Build VS Code extension (optional - run 'cargo make vscode-package' before CMake if needed)
add_custom_target(build_vscode_extension
    COMMAND npm install
    COMMAND npm run compile
    COMMAND npx @vscode/vsce package --no-dependencies
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/wixer-vscode
    COMMENT "Building VS Code extension"
)

# Define install location
install(PROGRAMS target/release/wixer.exe DESTINATION bin COMPONENT cli)

# VS Code Extension (optional component)
# Use glob to find the VSIX file regardless of version
file(GLOB VSCODE_VSIX_FILES "${CMAKE_SOURCE_DIR}/wixer-vscode/wixer-vscode-*.vsix")
if(VSCODE_VSIX_FILES)
    list(GET VSCODE_VSIX_FILES 0 VSCODE_VSIX_FILE)
    get_filename_component(VSCODE_VSIX_NAME "${VSCODE_VSIX_FILE}" NAME)
    install(FILES "${VSCODE_VSIX_FILE}"
        DESTINATION vscode
        COMPONENT vscode_extension)
    message(STATUS "VS Code extension: ${VSCODE_VSIX_NAME}")
else()
    message(STATUS "VS Code extension: not found (run 'cargo make vscode-package' first)")
endif()

# Configure and install AppxManifest for MSIX
if(WIN32)
    set(APPX_NAME "Natyamatsya.Wixer")
    set(APPX_VERSION "${PROJECT_VERSION}.0")

    configure_file(cmake/AppxManifest.xml.in ${CMAKE_BINARY_DIR}/AppxManifest.xml @ONLY)
    configure_file(cmake/MakeAppX.cmake.in ${CMAKE_BINARY_DIR}/MakeAppX.cmake @ONLY)

    install(FILES ${CMAKE_BINARY_DIR}/AppxManifest.xml DESTINATION .)
    install(DIRECTORY assets DESTINATION .)

    # Set up wixer impersonation
    include(WixerImpersonate)
    wixer_impersonate(
        WIXER_EXE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe"
        VERBOSE
        INSPECT_PDB
    )

    # Export main wixer.wxs.yaml to XML (pre-build step)
    # Note: We only export the root wixer.wxs.yaml, not the gallery examples
    set(WIXER_EXECUTABLE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe")
    set(WIXER_WXS_YAML "${CMAKE_SOURCE_DIR}/wixer.wxs.yaml")
    set(WIXER_WXS_XML "${CMAKE_BINARY_DIR}/wix/wixer.wxs")
    
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/wix")
    
    add_custom_command(
        OUTPUT "${WIXER_WXS_XML}"
        COMMAND "${WIXER_EXECUTABLE}" yaml-to-wix "${WIXER_WXS_YAML}" "${WIXER_WXS_XML}"
        DEPENDS "${WIXER_WXS_YAML}"
        COMMENT "Exporting wixer.wxs.yaml to XML"
        VERBATIM
    )
    
    add_custom_target(wixer_yaml_export
        DEPENDS "${WIXER_WXS_XML}"
    )
    
    set(WIXER_YAML_OUTPUT_DIR "${CMAKE_BINARY_DIR}/wix")
    set(WIXER_YAML_OUTPUT_FILES "${WIXER_WXS_XML}")

    # Configure wixer-based MSIX packaging (auto-generates AppxManifest from YAML)
    wixer_msix_package("${CMAKE_SOURCE_DIR}/wixer.wxs.yaml"
        APPX_NAME "Natyamatsya.Wixer"
        APPX_VERSION "${PROJECT_VERSION}.0"
    )
endif()

# CPack Configuration
set(CPACK_PACKAGE_NAME "Wixer")
set(CPACK_PACKAGE_VENDOR "Natyamatsya")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Wixer Installer Generator")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_INSTALL_DIRECTORY "Wixer")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_SOURCE_DIR}/LICENSE.txt")

# Component configuration
set(CPACK_COMPONENTS_ALL cli vscode_extension)
set(CPACK_COMPONENT_CLI_DISPLAY_NAME "Wixer CLI")
set(CPACK_COMPONENT_CLI_DESCRIPTION "Core wixer command-line tool")
set(CPACK_COMPONENT_CLI_REQUIRED ON)
set(CPACK_COMPONENT_VSCODE_EXTENSION_DISPLAY_NAME "VS Code Extension")
set(CPACK_COMPONENT_VSCODE_EXTENSION_DESCRIPTION "Language support for WiX YAML files in Visual Studio Code")
set(CPACK_COMPONENT_VSCODE_EXTENSION_DISABLED ON)

# WIX Generator (MSI) - Use WiX v4+
set(CPACK_WIX_VERSION "4")
set(CPACK_WIX_UPGRADE_GUID "d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a")
set(CPACK_WIX_COMPONENT_INSTALL ON)

# Package Generators
if(WIN32)
    set(CPACK_EXTERNAL_ENABLE_PACKAGING ON)
    set(CPACK_EXTERNAL_ENABLE_STAGING ON)
    
    # Select MSIX packaging strategy:
    # - "legacy": Use pre-configured AppxManifest.xml.in (MakeAppX.cmake)
    # - "wixer": Auto-generate AppxManifest from WiX YAML (MakeWixerMSIX.cmake)
    if(NOT DEFINED WIXER_MSIX_STRATEGY)
        set(WIXER_MSIX_STRATEGY "legacy" CACHE STRING "MSIX packaging strategy (legacy or wixer)")
    endif()
    
    if(WIXER_MSIX_STRATEGY STREQUAL "wixer")
        set(CPACK_EXTERNAL_PACKAGE_SCRIPT "${CMAKE_BINARY_DIR}/MakeWixerMSIX.cmake")
        message(STATUS "MSIX strategy: wixer (auto-generate AppxManifest from YAML)")
    else()
        set(CPACK_EXTERNAL_PACKAGE_SCRIPT "${CMAKE_BINARY_DIR}/MakeAppX.cmake")
        message(STATUS "MSIX strategy: legacy (use AppxManifest.xml.in)")
    endif()
    
    set(CPACK_GENERATOR "WIX;External")
else()
    set(CPACK_GENERATOR "TGZ")
endif()

include(CPack)

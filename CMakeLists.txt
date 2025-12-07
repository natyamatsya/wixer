cmake_minimum_required(VERSION 3.20)
project(wixer VERSION 0.1.0 LANGUAGES CXX)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")

# Build wixer using cargo
add_custom_target(build_wixer ALL
    COMMAND cargo build --release
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    COMMENT "Building wixer with cargo"
)

# Define install location
install(PROGRAMS target/release/wixer.exe DESTINATION bin)

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

    # Export YAML WiX sources to XML (pre-build step)
    set(WIXER_EXECUTABLE "${CMAKE_SOURCE_DIR}/target/release/wixer.exe")
    wixer_add_yaml_sources(wixer_yaml_export
        "${CMAKE_SOURCE_DIR}"
        "${CMAKE_BINARY_DIR}/wix"
    )

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

# WIX Generator (MSI) - Use WiX v4+
set(CPACK_WIX_VERSION "4")
set(CPACK_WIX_UPGRADE_GUID "d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a")

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

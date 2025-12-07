cmake_minimum_required(VERSION 3.20)
project(wixer VERSION 0.1.0 LANGUAGES CXX)

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
    
    # TODO: Add assets installation
    # install(DIRECTORY assets DESTINATION .)
endif()

# CPack Configuration
set(CPACK_PACKAGE_NAME "Wixer")
set(CPACK_PACKAGE_VENDOR "Natyamatsya")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Wixer Installer Generator")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_INSTALL_DIRECTORY "Wixer")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_SOURCE_DIR}/LICENSE")

# WIX Generator (MSI)
set(CPACK_WIX_UPGRADE_GUID "d0c5a0e0-5d2a-4b0e-9f0a-1c8b3d4e5f6a")
# set(CPACK_WIX_PRODUCT_ICON "${CMAKE_SOURCE_DIR}/wixer/assets/wixer.ico") 

# MSIX Generation (External)
if(WIN32)
    set(CPACK_EXTERNAL_ENABLE_PACKAGING ON)
    set(CPACK_EXTERNAL_PACKAGE_SCRIPT "${CMAKE_BINARY_DIR}/MakeAppX.cmake")
    set(CPACK_GENERATOR "WIX;External")
else()
    set(CPACK_GENERATOR "TGZ")
endif()

include(CPack)

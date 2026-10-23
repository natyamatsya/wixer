cmake_minimum_required(VERSION 3.25)
project(MyCppCustomAction VERSION 1.0.0 LANGUAGES CXX)

# MSI custom actions need the MSVC toolchain (static CRT, .def exports)
if(NOT MSVC)
    message(FATAL_ERROR "MyCppCustomAction requires MSVC (found ${CMAKE_CXX_COMPILER_ID}). "
        "Use the provided presets or configure from a Visual Studio Developer shell.")
endif()

# Modern C++23
set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Static link C++ runtime - no VCRedist dependency
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

# Build as DLL
add_library(MyCppCustomAction SHARED
    CustomAction.cpp
    CustomAction.def
)

# Windows-specific settings
target_compile_definitions(MyCppCustomAction PRIVATE
    WIN32_LEAN_AND_MEAN
    UNICODE
    _UNICODE
)

# Link against MSI API
target_link_libraries(MyCppCustomAction PRIVATE
    msi
)

# Export symbols via .def file
# Place the DLL directly in the build directory for every configuration
# (no Release/Debug subfolder), where Product.wxs.yaml expects it
set_target_properties(MyCppCustomAction PROPERTIES
    DEFINE_SYMBOL ""
    WINDOWS_EXPORT_ALL_SYMBOLS OFF
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}$<0:>"
)

# Install
install(TARGETS MyCppCustomAction
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
)

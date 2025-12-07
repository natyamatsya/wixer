cmake_minimum_required(VERSION 3.25)
project(MyCppCustomAction VERSION 1.0.0 LANGUAGES CXX)

# Modern C++23
set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

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
set_target_properties(MyCppCustomAction PROPERTIES
    DEFINE_SYMBOL ""
    WINDOWS_EXPORT_ALL_SYMBOLS OFF
)

# Install
install(TARGETS MyCppCustomAction
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
)

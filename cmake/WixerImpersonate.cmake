# WixerImpersonate.cmake
# Sets up wixer to impersonate wix.exe for transparent YAML support with CMake's WIX generator.
#
# Usage:
#   include(WixerImpersonate)
#   wixer_impersonate(
#       WIXER_EXE "/path/to/wixer.exe"
#       VERBOSE ON
#       INSPECT_PDB ON
#       INSPECT_SYMBOLS "symbol1;symbol2"
#       LOG_FILE "/path/to/wixer.log"
#   )
#
# This creates a wix.exe symlink/copy and wixer.yaml config, then sets the WIX
# environment variable so CMake's WIX generator uses wixer transparently.
#
# Options:
#   WIXER_EXE       - Path to wixer executable (auto-detected if not specified)
#   IMPERSONATE_DIR - Directory for impersonation files (default: ${CMAKE_BINARY_DIR}/_wixer_impersonate)
#   VERBOSE         - Enable verbose output from wixer
#   INSPECT_PDB     - Enable PDB inspection after build
#   INSPECT_SYMBOLS - List of symbols to inspect in PDB
#   LOG_FILE        - Path to log file for wixer output

function(wixer_impersonate)
    cmake_parse_arguments(ARG
        "VERBOSE;INSPECT_PDB"
        "WIXER_EXE;IMPERSONATE_DIR;LOG_FILE"
        "INSPECT_SYMBOLS"
        ${ARGN}
    )

    if(NOT ARG_WIXER_EXE)
        find_program(ARG_WIXER_EXE wixer)
        if(NOT ARG_WIXER_EXE)
            message(FATAL_ERROR "wixer executable not found. Specify WIXER_EXE or ensure wixer is in PATH.")
        endif()
    endif()

    if(NOT ARG_IMPERSONATE_DIR)
        set(ARG_IMPERSONATE_DIR "${CMAKE_BINARY_DIR}/_wixer_impersonate")
    endif()

    # Find real wix.exe
    find_program(_REAL_WIX_EXE wix
        PATHS
            "C:/Program Files/WiX Toolset v6.0/bin"
            "C:/Program Files/WiX Toolset v5.0/bin"
            "C:/Program Files/WiX Toolset v4.0/bin"
        ENV WIX
    )
    if(NOT _REAL_WIX_EXE)
        message(FATAL_ERROR "wix.exe not found. Install WiX Toolset or set WIX environment variable.")
    endif()

    # Set up impersonation directory
    file(MAKE_DIRECTORY "${ARG_IMPERSONATE_DIR}")

    # Create symbolic link to wixer.exe as wix.exe (falls back to copy)
    set(_IMPERSONATE_WIX_EXE "${ARG_IMPERSONATE_DIR}/wix.exe")
    if(NOT EXISTS "${_IMPERSONATE_WIX_EXE}")
        file(CREATE_LINK "${ARG_WIXER_EXE}" "${_IMPERSONATE_WIX_EXE}" SYMBOLIC COPY_ON_ERROR)
    endif()

    # Build wixer.yaml config
    set(_CONFIG_CONTENT "# Auto-generated wixer configuration\n")
    string(APPEND _CONFIG_CONTENT "build:\n")
    string(APPEND _CONFIG_CONTENT "  real_wix_path: \"${_REAL_WIX_EXE}\"\n")

    if(ARG_VERBOSE)
        string(APPEND _CONFIG_CONTENT "  verbose: true\n")
    endif()

    if(ARG_INSPECT_PDB)
        string(APPEND _CONFIG_CONTENT "  inspect_pdb: true\n")
    endif()

    if(ARG_INSPECT_SYMBOLS)
        string(APPEND _CONFIG_CONTENT "  inspect_symbols:\n")
        foreach(_sym ${ARG_INSPECT_SYMBOLS})
            string(APPEND _CONFIG_CONTENT "    - \"${_sym}\"\n")
        endforeach()
    endif()

    if(ARG_LOG_FILE)
        string(APPEND _CONFIG_CONTENT "logging:\n")
        string(APPEND _CONFIG_CONTENT "  file: \"${ARG_LOG_FILE}\"\n")
    endif()

    # Write wixer.yaml config
    set(_WIXER_CONFIG "${ARG_IMPERSONATE_DIR}/wixer.yaml")
    file(WRITE "${_WIXER_CONFIG}" "${_CONFIG_CONTENT}")

    # Set WIX environment variable to point to impersonation directory
    # CMake's WIX generator will find wix.exe there
    set(ENV{WIX} "${ARG_IMPERSONATE_DIR}")

    message(STATUS "Wixer impersonation enabled:")
    message(STATUS "  Wixer: ${ARG_WIXER_EXE}")
    message(STATUS "  Real WiX: ${_REAL_WIX_EXE}")
    message(STATUS "  WIX env: ${ARG_IMPERSONATE_DIR}")
    if(ARG_VERBOSE)
        message(STATUS "  Verbose: ON")
    endif()
    if(ARG_INSPECT_PDB)
        message(STATUS "  Inspect PDB: ON")
    endif()
    if(ARG_INSPECT_SYMBOLS)
        message(STATUS "  Inspect symbols: ${ARG_INSPECT_SYMBOLS}")
    endif()
    if(ARG_LOG_FILE)
        message(STATUS "  Log file: ${ARG_LOG_FILE}")
    endif()
endfunction()

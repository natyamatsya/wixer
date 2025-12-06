# Wixer.cmake - CMake module for integrating wixer
#
# Usage:
#   list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/path/to/wixer/cmake")
#   include(Wixer)
#
#   wixer_add_sources(
#       INPUT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/wix"
#       OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/wix_generated"
#       TARGET_NAME "wixer_gen"
#   )
#
#   # Use generated variables in CPack
#   if(WIXER_TEMPLATE_FILE)
#       set(CPACK_WIX_TEMPLATE "${WIXER_TEMPLATE_FILE}")
#   endif()
#   list(APPEND CPACK_WIX_EXTRA_SOURCES ${WIXER_GENERATED_FILES})

find_program(WIXER_EXECUTABLE wixer DOC "Path to wixer executable")

if(NOT WIXER_EXECUTABLE)
    message(WARNING "wixer executable not found. WiX generation will be disabled.")
endif()

function(wixer_add_sources)
    cmake_parse_arguments(ARG "" "INPUT_DIR;OUTPUT_DIR;TARGET_NAME" "" ${ARGN})

    if(NOT WIXER_EXECUTABLE)
        return()
    endif()

    if(NOT ARG_INPUT_DIR OR NOT ARG_OUTPUT_DIR)
        message(FATAL_ERROR "wixer_add_sources requires INPUT_DIR and OUTPUT_DIR")
    endif()

    if(NOT ARG_TARGET_NAME)
        set(ARG_TARGET_NAME "wixer_gen")
    endif()

    # Create output directory
    file(MAKE_DIRECTORY "${ARG_OUTPUT_DIR}")

    # Main export command
    # We use a timestamp file to track when the export was last run
    set(TIMESTAMP_FILE "${ARG_OUTPUT_DIR}/wixer_export.timestamp")

    # Glob sources to track dependencies
    # Note: CONFIGURE_DEPENDS requires CMake 3.12+
    file(GLOB_RECURSE WIXER_SOURCES CONFIGURE_DEPENDS
        "${ARG_INPUT_DIR}/*.wys"
        "${ARG_INPUT_DIR}/*.wyi"
        "${ARG_INPUT_DIR}/*.wyl"
        "${ARG_INPUT_DIR}/*.wys.in"
        "${ARG_INPUT_DIR}/*.wyi.in"
        "${ARG_INPUT_DIR}/*.wyl.in"
    )

    add_custom_command(
        OUTPUT "${TIMESTAMP_FILE}"
        COMMAND "${WIXER_EXECUTABLE}" export "${ARG_INPUT_DIR}" "${ARG_OUTPUT_DIR}"
        COMMAND "${CMAKE_COMMAND}" -E touch "${TIMESTAMP_FILE}"
        DEPENDS ${WIXER_SOURCES}
        COMMENT "Running wixer export on ${ARG_INPUT_DIR}"
        VERBATIM
    )

    add_custom_target("${ARG_TARGET_NAME}" ALL DEPENDS "${TIMESTAMP_FILE}")

    # Helper to predict output paths and populate variables
    set(_GENERATED_FILES "")
    set(_TEMPLATE_FILE "")

    foreach(SRC ${WIXER_SOURCES})
        file(RELATIVE_PATH REL_PATH "${ARG_INPUT_DIR}" "${SRC}")
        
        # Determine output path by replacing extensions
        # Note: string(REPLACE) replaces all occurrences. 
        # We want to be specific about extensions.
        
        set(OUT_PATH "${REL_PATH}")
        if(OUT_PATH MATCHES "\\.wys$")
            string(REPLACE ".wys" ".wxs" OUT_PATH "${OUT_PATH}")
        elseif(OUT_PATH MATCHES "\\.wyi$")
            string(REPLACE ".wyi" ".wxi" OUT_PATH "${OUT_PATH}")
        elseif(OUT_PATH MATCHES "\\.wyl$")
            string(REPLACE ".wyl" ".wxl" OUT_PATH "${OUT_PATH}")
        elseif(OUT_PATH MATCHES "\\.wys\\.in$")
            string(REPLACE ".wys.in" ".wxs.in" OUT_PATH "${OUT_PATH}")
        elseif(OUT_PATH MATCHES "\\.wyi\\.in$")
            string(REPLACE ".wyi.in" ".wxi.in" OUT_PATH "${OUT_PATH}")
        elseif(OUT_PATH MATCHES "\\.wyl\\.in$")
            string(REPLACE ".wyl.in" ".wxl.in" OUT_PATH "${OUT_PATH}")
        endif()
        
        set(ABS_OUT_PATH "${ARG_OUTPUT_DIR}/${OUT_PATH}")
        
        # Check if this is a template
        # CPack template is usually a .wxs file (or .wxs.in)
        if(OUT_PATH MATCHES "template\\.wxs\\.in$" OR OUT_PATH MATCHES "template\\.wxs$")
            set(_TEMPLATE_FILE "${ABS_OUT_PATH}")
        else()
            # Add to extra sources
            # CPack generally wants .wxs and .wxl in CPACK_WIX_EXTRA_SOURCES
            if(OUT_PATH MATCHES "\\.wxs$" OR OUT_PATH MATCHES "\\.wxl$")
                list(APPEND _GENERATED_FILES "${ABS_OUT_PATH}")
            endif()
        endif()
    endforeach()

    # Expose variables to parent scope
    set(WIXER_GENERATED_FILES "${_GENERATED_FILES}" PARENT_SCOPE)
    if(_TEMPLATE_FILE)
        set(WIXER_TEMPLATE_FILE "${_TEMPLATE_FILE}" PARENT_SCOPE)
    endif()

endfunction()

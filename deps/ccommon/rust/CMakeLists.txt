file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")

execute_process(
    COMMAND /usr/bin/env "CMAKE_BINARY_DIR=${CMAKE_BINARY_DIR}" "/bin/bash" "${CMAKE_CURRENT_LIST_DIR}/scripts/setup.sh"
    WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}"
    RESULT_VARIABLE CARGO_RESULT
    OUTPUT_VARIABLE CARGO_CONFIG_OUT
    ERROR_VARIABLE CARGO_CONFIG_OUT)

if(HAVE_RUST)
    add_subdirectory(ccommon_rs)
    add_subdirectory(cc_binding)
    add_subdirectory(ccommon-derive)
    add_subdirectory(ccommon-backend)
endif()

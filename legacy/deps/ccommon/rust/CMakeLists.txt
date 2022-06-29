file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")

execute_process(
    COMMAND /usr/bin/env "CMAKE_BINARY_DIR=${CMAKE_BINARY_DIR}" "/bin/bash" "${CMAKE_CURRENT_LIST_DIR}/scripts/setup.sh"
    WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}"
    RESULT_VARIABLE CARGO_RESULT
    OUTPUT_VARIABLE CARGO_CONFIG_OUT
    ERROR_VARIABLE CARGO_CONFIG_OUT)

if(HAVE_RUST)
    add_subdirectory(ccommon-array)
    add_subdirectory(ccommon-backend)
    add_subdirectory(ccommon-buffer)
    add_subdirectory(ccommon-channel)
    add_subdirectory(ccommon-derive)
    add_subdirectory(ccommon-log)
    add_subdirectory(ccommon-rs)
    add_subdirectory(ccommon-stats)
    add_subdirectory(ccommon-stream)
    add_subdirectory(ccommon-sys)
    
endif()

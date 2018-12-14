execute_process(COMMAND
    env "CMAKE_BINARY_DIR=${CMAKE_BINARY_DIR}" bash scripts/build-cargo-config.sh
    WORKING_DIRECTORY "$CMAKE_CURRENT_LIST_DIR}")
add_subdirectory(cdb_rs)

if (CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CARGO_CMD cargo build)
    set(TARGET_DIR "debug")
else ()
    set(CARGO_CMD cargo build --release)
    set(TARGET_DIR "release")
endif ()

set(CDB_RS_SO "${CMAKE_CURRENT_BINARY_DIR}/${TARGET_DIR}/libcdb_rs.so")

add_custom_target(cdb_rs ALL
    COMMENT "Compiling cdb module"
    COMMAND CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR} ${CARGO_CMD}
    COMMAND cp ${CDB_RS_SO} ${CMAKE_CURRENT_BINARY_DIR}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
set_target_properties(cdb_rs PROPERTIES LOCATION ${CMAKE_CURRENT_BINARY_DIR})

add_test(NAME cdb_rs
    COMMAND cargo test
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})

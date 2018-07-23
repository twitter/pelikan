file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")
cargo_build(NAME ccommon_rs)
add_dependencies(ccommon_rs_static ccommon-static)
add_dependencies(ccommon_rs_shared ccommon-shared)

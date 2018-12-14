file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")
cargo_build(NAME ccommon_rs)
add_dependencies(ccommon_rs ccommon-static)

file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")

cargo_build(NAME ccommon_rs)

add_dependencies(ccommon_rs cc_binding)
add_dependencies(ccommon_rs ccommon-derive)
add_dependencies(ccommon_rs ccommon-backend)

target_link_libraries(ccommon_rs INTERFACE cc_binding ccommon-derive)

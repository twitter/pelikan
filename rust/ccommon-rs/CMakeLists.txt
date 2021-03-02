file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")

cargo_build(NAME ccommon-rs)

add_dependencies(ccommon-rs ccommon-sys)
add_dependencies(ccommon-rs ccommon-derive)
add_dependencies(ccommon-rs ccommon-backend)

target_link_libraries(ccommon-rs INTERFACE ccommon-sys ccommon-derive)

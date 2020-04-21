cargo_build(NAME ccommon-sys)

target_link_libraries(ccommon-sys INTERFACE ccommon-static)

add_dependencies(ccommon-sys ccommon-static)

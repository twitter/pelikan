cargo_build(NAME cc_binding)

target_link_libraries(cc_binding INTERFACE ccommon-static)

add_dependencies(cc_binding ccommon-static)
add_dependencies(cc_binding import-link-flags)


cargo_build(NAME ccommon-derive)

add_dependencies(ccommon-derive import-link-flags)

target_link_libraries(ccommon-derive INTERFACE import-link-flags)

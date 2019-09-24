
cargo_build(NAME pelikan_sys)

add_dependencies(pelikan_sys util time core)
add_dependencies(pelikan_sys cdb_rs slab cuckoo)
add_dependencies(pelikan_sys client-network)
add_dependencies(pelikan_sys protocol_memcache protocol_ping protocol_resp protocol_resp_tw protocol_admin)
add_dependencies(pelikan_sys cc_bindings)
add_dependencies(pelikan_sys ccommon-static)

target_link_libraries(pelikan_sys INTERFACE util time core)
target_link_libraries(pelikan_sys INTERFACE cdb_rs slab cuckoo)
target_link_libraries(pelikan_sys INTERFACE client-network protocol_memcache procotol_ping protocol_resp protocol_resp_tw protocol_admin)
target_link_libraries(pelikan_sys INTERFACE cc_bindings)

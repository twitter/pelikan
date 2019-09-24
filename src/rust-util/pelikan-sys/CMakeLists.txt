
cargo_build(NAME pelikan_sys)

add_dependencies(pelikan_sys util time core)
add_dependencies(pelikan_sys cdb_rs slab cuckoo)
add_dependencies(pelikan_sys client-network)
add_dependencies(pelikan_sys protocol_memcache protocol_ping protocol_resp protocol_resp_tw protocol_admin)
add_dependencies(pelikan_sys cc_bindings)

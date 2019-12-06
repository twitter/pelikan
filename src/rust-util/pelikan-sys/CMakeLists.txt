
cargo_build(NAME pelikan_sys)

add_dependencies(pelikan_sys util time core)
add_dependencies(pelikan_sys slab cuckoo)
add_dependencies(pelikan_sys client-network)
add_dependencies(pelikan_sys protocol_memcache protocol_ping protocol_resp protocol_admin)
add_dependencies(pelikan_sys cc_binding)
add_dependencies(pelikan_sys hotkey)
add_dependencies(pelikan_sys datapool)

if (TARGET_CDB)
    add_dependencies(pelikan_sys cdb_rs)
    target_link_libraries(pelikan_sys INTERFACE cdb_rs)
endif()

target_link_libraries(pelikan_sys INTERFACE util time core)
target_link_libraries(pelikan_sys INTERFACE slab cuckoo)
target_link_libraries(pelikan_sys INTERFACE client-network protocol_memcache protocol_ping protocol_resp protocol_admin)
target_link_libraries(pelikan_sys INTERFACE cc_binding)
target_link_libraries(pelikan_sys INTERFACE hotkey)
target_link_libraries(pelikan_sys INTERFACE datapool)

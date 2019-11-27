cargo_build(NAME pelikan_dual BIN)

add_dependencies(pelikan_dual rustcore)

target_link_libraries(pelikan_dual rustcore)

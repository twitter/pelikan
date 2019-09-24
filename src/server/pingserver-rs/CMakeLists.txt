
cargo_build(NAME pelikan_pingserver_rs BIN)

add_dependencies(pelikan_pingserver_rs pelikan_sys pelikan_rs)

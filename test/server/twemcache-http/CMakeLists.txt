
cargo_build(NAME check_twemcache_http NO_TEST BIN)

add_test(
    NAME dual-protocol-integration
    COMMAND check_pingserver_rs "${CMAKE_BINARY_DIR}"
    TIMEOUT 10
)

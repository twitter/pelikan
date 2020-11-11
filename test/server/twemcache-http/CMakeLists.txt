
cargo_build(NAME check_twemcache_http NO_TEST BIN)

add_test(
    NAME twemcache-http-integration
    COMMAND check_twemcache_http "${CMAKE_BINARY_DIR}"
    TIMEOUT 10
)

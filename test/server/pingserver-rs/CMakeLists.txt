
cargo_build(NAME check_pingserver_rs NO_TEST BIN)

add_test(
    NAME pingserver-rs-integration
    COMMAND check_pingserver_rs "${CMAKE_BINARY_DIR}"
    TIMEOUT 10
)

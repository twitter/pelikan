
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${PROJECT_BINARY_DIR}/_bin)
set(TARGET_NAME pelikan_twemcache_rs)

cargo_build(NAME ${TARGET_NAME} BIN)

install(TARGETS ${TARGET_NAME} RUNTIME DESTINATION bin)
add_dependencies(service ${TARGET_NAME})

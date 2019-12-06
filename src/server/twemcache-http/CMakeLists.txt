
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${PROJECT_BINARY_DIR}/_bin)
set(TARGET_NAME pelikan_twemcache_http)

cargo_build(NAME ${TARGET_NAME} BIN)

add_dependencies(${TARGET_NAME} rustcore)
add_dependencies(${TARGET_NAME} pelikan_rs)

target_link_libraries(${TARGET_NAME} rustcore)
target_link_libraries(${TARGET_NAME} pelikan_rs)
target_link_libraries(${TARGET_NAME} datapool)

install(TARGETS ${TARGET_NAME} RUNTIME DESTINATION bin)
add_dependencies(service ${TARGET_NAME})

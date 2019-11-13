
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${PROJECT_BINARY_DIR}/_bin)
set(TARGET_NAME pelikan_pingserver_rs)

cargo_build(NAME ${TARGET_NAME} BIN)

add_dependencies(${TARGET_NAME} pelikan_sys pelikan_rs rustcore)
target_link_libraries(${TARGET_NAME} pelikan_sys pelikan_rs rustcore)

install(TARGETS ${TARGET_NAME} RUNTIME DESTINATION bin)
add_dependencies(service ${TARGET_NAME})

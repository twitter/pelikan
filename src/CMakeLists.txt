add_subdirectory(core ${PROJECT_BINARY_DIR}/core)
add_subdirectory(hotkey ${PROJECT_BINARY_DIR}/hotkey)
add_subdirectory(protocol ${PROJECT_BINARY_DIR}/protocol)
add_subdirectory(storage ${PROJECT_BINARY_DIR}/storage)
add_subdirectory(time ${PROJECT_BINARY_DIR}/time)
add_subdirectory(util ${PROJECT_BINARY_DIR}/util)

# executables
add_custom_target(service)
add_subdirectory(server ${PROJECT_BINARY_DIR}/server)

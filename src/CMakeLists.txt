add_subdirectory(core ${PROJECT_BINARY_DIR}/core)
add_subdirectory(protocol ${PROJECT_BINARY_DIR}/protocol)
add_subdirectory(storage ${PROJECT_BINARY_DIR}/storage)
add_subdirectory(time ${PROJECT_BINARY_DIR}/time)
add_subdirectory(util ${PROJECT_BINARY_DIR}/util)

if(TARGET_SLIMCACHE)
    add_subdirectory(slimcache)
endif()

if(TARGET_TWEMCACHE)
    add_subdirectory(twemcache)
endif()

if(TARGET_REDIS)
    add_subdirectory(redis)
endif()

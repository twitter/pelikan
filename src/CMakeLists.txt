add_subdirectory(protocol ${PROJECT_BINARY_DIR}/protocol)
add_subdirectory(storage ${PROJECT_BINARY_DIR}/storage)
add_subdirectory(time ${PROJECT_BINARY_DIR}/time)

if(TARGET_SLIMCACHE)
add_subdirectory(slimcache)
endif()

if(TARGET_TWEMCACHE)
add_subdirectory(twemcache)
endif()

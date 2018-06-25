add_subdirectory(admin)
add_subdirectory(data)

set(SOURCE
    ${SOURCE}
    main.c
    setting.c
    stats.c)

set(MODULES
    cdb_ffi_shared
    core
    protocol_admin
    protocol_memcache
    slab
    time
    util)

set(LIBS
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT}
    )

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${PROJECT_BINARY_DIR}/_bin)
set(TARGET_NAME ${PROJECT_NAME}_cdb)

add_executable(${TARGET_NAME} ${SOURCE})
target_link_libraries(${TARGET_NAME} ${MODULES} ${LIBS})
add_dependencies(${TARGET_NAME} cdb_ffi_shared)

install(TARGETS ${TARGET_NAME} RUNTIME DESTINATION bin)
add_dependencies(service ${TARGET_NAME})

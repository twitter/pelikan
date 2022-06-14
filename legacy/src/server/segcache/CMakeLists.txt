add_subdirectory(admin)
add_subdirectory(data)

set(SOURCE
    ${SOURCE}
    main.c
    setting.c
    stats.c)

set(MODULES
    core
    hotkey
    protocol_admin
    protocol_memcache
    seg
    time
    util)

set(LIBS
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${PROJECT_BINARY_DIR}/_bin)
set(TARGET_NAME ${PROJECT_NAME}_segcache)

add_executable(${TARGET_NAME} ${SOURCE})
target_link_libraries(${TARGET_NAME} ${MODULES} ${LIBS})

install(TARGETS ${TARGET_NAME} RUNTIME DESTINATION bin)
add_dependencies(service ${TARGET_NAME})

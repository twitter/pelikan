add_subdirectory(admin)
add_subdirectory(data)

set(SOURCE
    ${SOURCE}
    main.c
    stats.c)

set(MODULES
    core
    protocol_admin
    protocol_memcache
    slab
    time
    util)

set(LIBS
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(${PROJECT_NAME}_twemcache ${SOURCE})
target_link_libraries(${PROJECT_NAME}_twemcache ${MODULES} ${LIBS})

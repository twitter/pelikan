set(SOURCE
    main.c
    process.c)

set(LIBS
    core
    slab
    memcache
    time
    util
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(${PROJECT_NAME}_twemcache ${SOURCE})
target_link_libraries(${PROJECT_NAME}_twemcache ${LIBS})

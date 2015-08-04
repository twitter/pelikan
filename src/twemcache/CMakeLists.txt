set(SOURCE
    main.c
    process.c
    core_worker.c)

set(LIBS
    core
    slab
    memcache
    time
    util
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

add_executable(${PROJECT_NAME}_twemcache ${SOURCE})
target_link_libraries(${PROJECT_NAME}_twemcache ${LIBS})

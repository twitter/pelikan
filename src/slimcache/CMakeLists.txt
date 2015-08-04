set(SOURCE
    main.c
    process.c
    core_worker.c)

set(LIBS
    core
    cuckoo
    memcache
    time
    util
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

add_executable(${PROJECT_NAME}_slimcache ${SOURCE})
target_link_libraries(${PROJECT_NAME}_slimcache ${LIBS})

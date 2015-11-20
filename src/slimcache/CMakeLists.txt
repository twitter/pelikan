set(SOURCE
    admin_helper.c
    main.c
    process.c
    worker_helper.c )

set(LIBS
    admin
    core
    cuckoo
    memcache
    time
    util
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(${PROJECT_NAME}_slimcache ${SOURCE})
target_link_libraries(${PROJECT_NAME}_slimcache ${LIBS})

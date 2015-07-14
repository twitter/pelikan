set(SOURCE
    main.c
    process.c)

set(LIBS
    core
    cuckoo
    memcache
    time
    util
    ccommon-static)

add_executable(${PROJECT_NAME}_slimcache ${SOURCE})
target_link_libraries(${PROJECT_NAME}_slimcache ${LIBS})

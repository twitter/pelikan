set(SOURCE
    main.c
    process.c)

set(LIBS
    ping
    time
    util
    ccommon-static)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(${PROJECT_NAME}_pingserver ${SOURCE})
target_link_libraries(${PROJECT_NAME}_pingserver ${LIBS})

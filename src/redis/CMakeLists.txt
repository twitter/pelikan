set(SOURCE
    main.c)

set(LIBS
    time
    util
    ccommon-static)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(${PROJECT_NAME}_redis ${SOURCE})
target_link_libraries(${PROJECT_NAME}_redis ${LIBS})

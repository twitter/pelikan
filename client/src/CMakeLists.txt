set(source client.c
           client_core.c)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(${PROJECT_NAME}_client ${source})
target_link_libraries(${PROJECT_NAME}_client ccommon-static ${CLIENT_LIBRARIES})

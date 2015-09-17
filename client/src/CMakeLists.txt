set(source client.c
           client_core.c)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/_bin)
add_executable(broadbill_client ${source})
target_link_libraries(broadbill_client ccommon-static ${CLIENT_LIBRARIES})

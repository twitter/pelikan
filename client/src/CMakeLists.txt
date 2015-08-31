set(source client.c
           client_core.c)

add_executable(broadbill_client ${source})
target_link_libraries(broadbill_client ccommon-static ${CLIENT_LIBRARIES})

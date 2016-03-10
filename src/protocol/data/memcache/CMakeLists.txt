set(SOURCE
    request.c
    response.c
    parse.c
    compose.c
    klog.c)

add_library(protocol_memcache ${SOURCE})
target_link_libraries(protocol_memcache ccommon-static)

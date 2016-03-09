set(SOURCE
    request.c
    response.c
    parse.c
    compose.c
    klog.c)

add_library(protocol_memcache ${SOURCE})

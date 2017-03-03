set(SOURCE
    request.c
    response.c
    token.c)

add_library(protocol_redis ${SOURCE})

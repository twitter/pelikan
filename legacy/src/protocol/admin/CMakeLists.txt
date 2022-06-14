set(SOURCE
    format.c
    compose.c
    parse.c
    request.c
    response.c)

add_library(protocol_admin ${SOURCE})

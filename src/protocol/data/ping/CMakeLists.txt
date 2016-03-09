set(SOURCE
    request.c
    response.c
    parse.c
    compose.c)

add_library(protocol_ping ${SOURCE})

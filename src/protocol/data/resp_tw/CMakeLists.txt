
set(SOURCE
    token.c
    request.c
    response.c
    compose.c
    parse.c)

add_library(protocol_resp_tw ${SOURCE})

set(SOURCE
    attribute.c
    compose.c
    parse.c
    request.c
    response.c
    token.c)

add_library(protocol_resp ${SOURCE})

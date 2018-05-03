#include "cli_network.h"

#include <netdb.h>

struct addrinfo hints;
struct addrinfo *ai = NULL;

bool
cli_connect(struct buf_sock *client, char *host, char *port)
{
    hints.ai_flags = AI_NUMERICSERV;
    hints.ai_family = PF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;

    getaddrinfo(host, port, &hints, &ai);
    if (client->hdl->open(ai, client->ch)) {
        /* TODO: make socket blocking */
        return true;
    } else {
        return false;
    }
}


void
cli_disconnect(struct buf_sock *client)
{
    client->hdl->term(client->ch);
}

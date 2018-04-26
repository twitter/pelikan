#include "cli_network.h"

#include <channel/cc_tcp.h>

#include <stdio.h>


static struct tcp_conn *conn = NULL;

struct addrinfo hints;
struct addrinfo *ai = NULL;

bool
cli_connect(char *host, uint16_t port)
{
    char servname[CC_UINTMAX_MAXLEN + 1];

    sprintf(servname, "%"PRIu32, port);

    conn = tcp_conn_create();
    if (conn == NULL) {
        return false;
    }

    hints.ai_flags = AI_NUMERICSERV;
    hints.ai_family = PF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;

    getaddrinfo(host, servname, &hints, &ai);
    if (tcp_connect(ai, conn)) {
        /* TODO: make socket blocking */
        return true;
    } else {
        return false;
    }
}


void
cli_disconnect(void)
{
    tcp_close(conn);
}

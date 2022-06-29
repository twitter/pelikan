#include "cli_network.h"

#include "core/data/server.h"

#include <channel/cc_channel.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

#include <netdb.h>

struct addrinfo hints;
struct addrinfo *ai = NULL;

struct network_config network_config = {LOCAL, NULL, SERVER_PORT};

channel_handler_st tcp_handler = {
    .accept = NULL,
    .reject = NULL,
    .open = (channel_open_fn)tcp_connect,
    .term = (channel_term_fn)tcp_close,
    .recv = (channel_recv_fn)tcp_recv,
    .send = (channel_send_fn)tcp_send,
    .rid = (channel_id_fn)tcp_read_id,
    .wid = (channel_id_fn)tcp_write_id
};

bool
cli_connect(struct buf_sock *client)
{
    hints.ai_flags = AI_NUMERICSERV;
    hints.ai_family = PF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;

    getaddrinfo(network_config.host, network_config.port, &hints, &ai);
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

bool
cli_reconnect(struct buf_sock *client)
{
    cli_disconnect(client);
    fwrite(DISCONNECT_MSG, sizeof(DISCONNECT_MSG), 1, stdout);
    if (!cli_connect(client)) {
        network_config.mode = OFFLINE;
        return false;
    } else {
        fwrite(RECONNECT_MSG, sizeof(RECONNECT_MSG), 1, stdout);
        return true;
    }
}

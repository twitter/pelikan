#pragma once

/* for CLI, a few things are simplified:
 *  - we only need one connection, so we keep it as a static global variable in
 *   the network module.
 *  - retry and timeout policy are coded into the network module as well, since
 *   we don't expect many edge cases (mostly used on localhost for testing or
 *   debuggin)
 *  - network IO should block
 */

#include <stream/cc_sockio.h>

#include <stdbool.h>
#include <stdint.h>

/* string argument in order: protocol, host, port */
#define PROMPT_FMT_OFFLINE "%s %s:%s (not connected) > "
#define PROMPT_FMT_LOCAL "%s :%s > " /* show protocol & port */
#define PROMPT_FMT_REMOTE "%s %s: > " /* show protocol & host */

#define SEND_ERROR "ERROR SENDING REQUEST\r\n"
#define RECV_ERROR "ERROR RECEIVING RESPONSE\r\n"
#define RECV_HUP "SERVER HUNG UP (e.g. due to syntax error)\r\n"
#define DISCONNECT_MSG "CLIENT DISCONNECTED\r\n"
#define RECONNECT_MSG "CLIENT RECONNECTED\r\n"

typedef enum cli_network {
    LOCAL = 0,
    REMOTE = 1,
    OFFLINE = 2,
} cli_network_e;

struct network_config {
    cli_network_e   mode;
    char *          host;
    char *          port;
};

extern channel_handler_st tcp_handler;
extern struct network_config network_config;

/* network_config is used for cli_connect */
bool cli_connect(struct buf_sock *client);
void cli_disconnect(struct buf_sock *client);
bool cli_reconnect(struct buf_sock *client);

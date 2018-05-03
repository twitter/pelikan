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

/* if host is NULL, loopback address will be used */
bool cli_connect(struct buf_sock *client, char *host, char *port);
void cli_disconnect(struct buf_sock *client);

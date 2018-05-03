#include "cli.h"


#include "../network/cli_network.h"

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_print.h>
#include <channel/cc_channel.h>
#include <stream/cc_sockio.h>

#include <sys/param.h>

#define PROTOCOL "resp"
#define PROMPT_FMT_OFFLINE PROTOCOL " %s:%s (not connected) > "
#define PROMPT_FMT_LOCAL PROTOCOL " :%s > " /* use port */
#define PROMPT_FMT_REMOTE PROTOCOL " %s: > " /* use host */

#define IO_BUF_MAX 1024

typedef enum cli_mode {
    LOCAL = 0,
    REMOTE = 1,
    OFFLINE = 2,
} cli_mode_e;

struct config {
    cli_mode_e  mode;
    char *      host;
    char *      port;
};

struct iobuf {
    char        *input;
    char        *output;
    size_t      ilen;
    size_t      olen;
};

struct config config = {LOCAL, NULL, SERVER_PORT};
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
struct iobuf buf;

struct request *req;
struct response *rsp;
struct buf_sock *client;

void
cli_setup(rediscli_options_st *options)
{
    if (options != NULL) {
        config.host = options->server_host.val.vstr;
        config.port = options->data_port.val.vstr;
        if (config.host == NULL) { /* if host is not provided it's local */
            config.mode = LOCAL;
        } else {
            config.mode = REMOTE;
        }
    }

    /* slacking on NULL check here because this is very unlikely to fail */
    req = request_create();
    rsp = response_create();
    client = buf_sock_create();
    client->hdl = &tcp_handler;

}

void
cli_teardown(void)
{
    request_destroy(&req);
    response_destroy(&rsp);
    buf_sock_destroy(&client);
}

static void
_cli_prompt(void)
{
    size_t len;

    if (buf.output == NULL) {
        buf.output = cc_alloc(IO_BUF_MAX);
    }

    switch (config.mode) {
        case LOCAL:
            len = cc_snprintf(buf.output, IO_BUF_MAX, PROMPT_FMT_LOCAL,
                    config.port);
            buf.olen = MIN(len, IO_BUF_MAX - 1);
            break;

        case REMOTE:
            len = cc_snprintf(buf.output, IO_BUF_MAX, PROMPT_FMT_REMOTE,
                    config.host);
            buf.olen = MIN(len, IO_BUF_MAX - 1);
            break;

        case OFFLINE:
            len = cc_snprintf(buf.output, IO_BUF_MAX, PROMPT_FMT_OFFLINE,
                    (config.host == NULL) ? "localhost" : config.host,
                    config.port);
            buf.olen = MIN(len, IO_BUF_MAX - 1);
            break;

        default:
            NOT_REACHED();
    }
}


static void
_cli_parse_req(void)
{
    char *p, *token;
    struct element *el;

    /* do  not parse fully, just breaking down fields/tokens by delimiter */
    while ((token = strsep(&p, " \t")) != NULL) {
        el = array_push(req->token);
        el->type = ELEM_BULK;
        el->bstr.len = strlen(token);
        el->bstr.data = token;
    }
}


void
cli_run(void)
{
    bool quit = false;
    int status;

    if (!cli_connect(client, config.host, config.port)) {
        config.mode = OFFLINE;
    }

    while (!quit) {
        /* print prompt */
        _cli_prompt();
        fwrite(buf.output, buf.olen, 1, stdout);

        /* wait for input */
        getline(&buf.input, &buf.ilen, stdin);

        /* parse input buffer into request object, translate */
        _cli_parse_req();
        status = compose_req(&client->wbuf, req);
        if (status < 0) {
            /* TODO: handle error */
        }

        /* issue command */
        do {
            status = buf_tcp_write(client);
        } while (status == CC_ERETRY || status == CC_EAGAIN); /* retry write */
        if (status != CC_OK) {
            /* TODO: reset connection */
        }

        /* wait for complete response */
        do {
            buf_tcp_read(client);
            status = parse_rsp(rsp, client->rbuf);
        } while (status == PARSE_EUNFIN);
        client->rbuf->rpos = client->rbuf->begin;
        fwrite(client->rbuf->begin, buf_rsize(client->rbuf), 1, stdout);

        /* reset buffers and go to the top */
        buf_reset(client->rbuf);
        buf_reset(client->wbuf);
    }
}

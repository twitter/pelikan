#include "cli.h"

#include "../network/cli_network.h"

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_print.h>

#include <ctype.h>
#include <sys/param.h>

#define PROTOCOL "resp"
#define IO_BUF_MAX 1024

struct iobuf {
    char        *input;
    char        *output;
    size_t      ilen;
    size_t      olen;
};

bool quit = false;
struct iobuf buf;

struct request *req;
struct response *rsp;
struct buf_sock *client;

void
cli_setup(respcli_options_st *options)
{
    if (options != NULL) {
        network_config.host = options->server_host.val.vstr;
        network_config.port = options->data_port.val.vstr;
        if (network_config.host == NULL) { /* if host is not provided it's local */
            network_config.mode = LOCAL;
        } else {
            network_config.mode = REMOTE;
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

    switch (network_config.mode) {
        case LOCAL:
            len = cc_snprintf(buf.output, IO_BUF_MAX, PROMPT_FMT_LOCAL,
                    PROTOCOL, network_config.port);
            buf.olen = MIN(len, IO_BUF_MAX - 1);
            break;

        case REMOTE:
            len = cc_snprintf(buf.output, IO_BUF_MAX, PROMPT_FMT_REMOTE,
                    PROTOCOL, network_config.host);
            buf.olen = MIN(len, IO_BUF_MAX - 1);
            break;

        case OFFLINE:
            len = cc_snprintf(buf.output, IO_BUF_MAX, PROMPT_FMT_OFFLINE,
                    PROTOCOL, (network_config.host == NULL) ? "localhost" :
                    network_config.host, network_config.port);
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

    p = buf.input;
    /* do  not parse fully, just breaking down fields/tokens by delimiter */
    while ((token = strsep(&p, " \t\r\n")) != NULL) {
        if (isspace(*token) || *token == '\0') {
            continue;
        }
        el = array_push(req->token);
        el->type = ELEM_BULK;
        el->bstr.len = strlen(token);
        el->bstr.data = token;
    }
}

static bool
_cli_onerun(void)
{
    int status;

    buf_reset(client->rbuf);
    buf_reset(client->wbuf);
    request_reset(req);
    response_reset(rsp);

    /* print prompt */
    _cli_prompt();
    fwrite(buf.output, buf.olen, 1, stdout);

    /* wait for input, quit to exit the loop */
    getline(&buf.input, &buf.ilen, stdin);
    if (cc_strncmp(buf.input, "quit", 4) == 0) {
        quit = true;
        return true;
    }

    /* parse input buffer into request object, translate */
    _cli_parse_req();
    status = compose_req(&client->wbuf, req);
    if (status < 0) {
        /* TODO: handle OOM error */
    }

    /* issue command */
    do {
        status = buf_tcp_write(client);
    } while (status == CC_ERETRY || status == CC_EAGAIN); /* retry write */
    if (status != CC_OK) {
        fwrite(SEND_ERROR, sizeof(SEND_ERROR), 1, stdout);
        return false;
    }

    /* wait for complete response */
    do {
        status = buf_tcp_read(client);
        if (status != CC_OK && status != CC_ERETRY) {
            if (status == CC_ERDHUP) {
                fwrite(RECV_HUP, sizeof(RECV_HUP), 1, stdout);
            } else {
                fwrite(RECV_ERROR, sizeof(RECV_ERROR), 1, stdout);
            }
            return false;
        }
        status = parse_rsp(rsp, client->rbuf);
    } while (status == PARSE_EUNFIN);
    client->rbuf->rpos = client->rbuf->begin;
    fwrite(client->rbuf->begin, buf_rsize(client->rbuf), 1, stdout);

    return true;
}


void
cli_run(void)
{
    if (!cli_connect(client)) {
        network_config.mode = OFFLINE;
    }

    while (!quit) {
        if (!_cli_onerun() && !cli_reconnect(client)) {
            /* should reconnect but it failed */
            quit = true;
        }
    }

}

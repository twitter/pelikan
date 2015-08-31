#include <client_core.h>

#include <cc_define.h>
#include <channel/cc_tcp.h>

#include <stdio.h>

#define GET_CMD_FMT "get %s\r\n"
#define SET_CMD_FMT "set %s 0 0 %u\r\n%s\r\n"

struct tcp_conn connection;
struct tcp_conn *c = &connection;

rstatus_t
client_core_setup(struct addrinfo *ai)
{
    if (!tcp_connect(ai, c)) {
        log_error("Could not connect to server!");
        return CC_ERROR;
    }

    return CC_OK;
}

void
client_core_teardown(void)
{
    tcp_close(c);
}

static rstatus_t
client_core_send(char *buf, size_t nbyte)
{
    ssize_t ret;

    ret = tcp_send(c, buf, nbyte);

    while (ret == CC_EAGAIN) {
        ret = tcp_send(c, buf, nbyte);
    }

    if (ret < nbyte) {
        log_error("Could not send %u bytes!", nbyte);
        return CC_ERROR;
    }

    return CC_OK;
}

static ssize_t
client_core_recv(char *buf, size_t nbyte)
{
    ssize_t ret;

    ret = tcp_recv(c, buf, nbyte);

    while (ret == CC_EAGAIN) {
        ret = tcp_recv(c, buf, nbyte);
    }

    return ret;
}

static void
client_core_cmd(char *cmd, size_t nbyte)
{
    ssize_t ret;
    char recv_buf[MiB];

    client_core_send(cmd, nbyte);
    ret = client_core_recv(recv_buf, MiB);

    if (ret < 0) {
        log_error("Could not recv server response!");
        return;
    }

    log_info("Server response: %.*s", ret, recv_buf);
}

void
client_core_run(void)
{
    size_t len;
    char send_buf[MiB];

    log_info("Setting key foo val bar");
    len = snprintf(send_buf, MiB, SET_CMD_FMT, "foo", 3, "bar");
    client_core_cmd(send_buf, len);

    log_info("Getting key foo");
    len = snprintf(send_buf, MiB, GET_CMD_FMT, "foo");
    client_core_cmd(send_buf, len);
}

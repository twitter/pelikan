#include "process.h"

#include "protocol/data/ping_include.h"

#include <buffer/cc_dbuf.h>
#include <cc_debug.h>

int
pingserver_process_read(struct buf_sock *s)
{
    parse_rstatus_t status;

    log_verb("post-read processing");

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(s->rbuf) > 0) {
        log_verb("%"PRIu32" bytes left", buf_rsize(s->rbuf));

        status = parse_req(s->rbuf);
        if (status == PARSE_EUNFIN) {
            return 0;
        }
        if (status != PARSE_OK) {
            return -1;
        }

        if (compose_rsp(&s->wbuf) != COMPOSE_OK) {
            return -1;
        }
    }

    return 0;
}

int
pingserver_process_write(struct buf_sock *s)
{
    log_verb("post-write processing");

    dbuf_shrink(&s->rbuf);
    dbuf_shrink(&s->wbuf);

    return 0;
}

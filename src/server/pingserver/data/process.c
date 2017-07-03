#include "process.h"

#include "protocol/data/ping_include.h"

#include <buffer/cc_dbuf.h>
#include <cc_debug.h>

int
pingserver_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{
    parse_rstatus_t status;

    log_verb("post-read processing");

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(*rbuf) > 0) {
        log_verb("%"PRIu32" bytes left", buf_rsize(*rbuf));

        status = parse_req(*rbuf);
        if (status == PARSE_EUNFIN) {
            return 0;
        }
        if (status != PARSE_OK) {
            return -1;
        }

        if (compose_rsp(wbuf) != COMPOSE_OK) {
            return -1;
        }
    }

    return 0;
}

int
pingserver_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    dbuf_shrink(rbuf);
    dbuf_shrink(wbuf);

    return 0;
}

int
pingserver_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}

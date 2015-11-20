#include <protocol/admin_include.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>
#include <buffer/cc_buf.h>

void
admin_post_read(struct buf_sock *s)
{
    parse_rstatus_t status;
    struct op *op;

    if (s->data == NULL) {
        s->data = op_create();
    }

    op = s->data;

    if (op == NULL) {
        goto error;
    }

    while (buf_rsize(s->rbuf) > 0) {
        struct reply *rep;
        int n;

        status = parse_op(op, s->rbuf);
        if (status == PARSE_EUNFIN) {
            goto done;
        }

        if (status != PARSE_OK) {
            log_info("illegal request received on admin port status %d",
                     status);
            goto error;
        }

        /* processing */
        if (op->type == OP_QUIT) {
            log_info("peer called quit");
            s->ch->state = CHANNEL_TERM;
            goto done;
        }

        /* no chained replies for now */
        rep = reply_create();
        if (rep == NULL) {
            log_error("could not allocate reply object");
            goto error;
        }

        process_op(rep, op);

        n = compose_rep(&s->wbuf, rep);
        if (n < 0) {
            log_error("compose reply error");
            reply_destroy(&rep);
            goto error;
        }

        op_reset(op);
        reply_destroy(&rep);
    }

    done:
    if (buf_rsize(s->wbuf) > 0) {
        admin_event_write(s);
    }
    return;

    error:
    s->ch->state = CHANNEL_TERM;
}

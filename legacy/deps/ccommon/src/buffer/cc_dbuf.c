#include <buffer/cc_dbuf.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>

#include <stddef.h>

#define DBUF_MODULE_NAME "ccommon::buffer::dbuf"

static bool dbuf_init = false;

/* Maximum size of the buffer */
static uint8_t max_power = DBUF_DEFAULT_MAX;
static uint32_t max_size = BUF_INIT_SIZE << DBUF_DEFAULT_MAX;
dbuf_metrics_st *dbuf_metrics = NULL;

void
dbuf_setup(dbuf_options_st *options, dbuf_metrics_st *metrics)
{
    log_info("set up the %s module", DBUF_MODULE_NAME);

    if (dbuf_init) {
        log_warn("%s has already been setup, overwrite", DBUF_MODULE_NAME);
    }

    dbuf_metrics = metrics;

    if (options != NULL) {
        /* TODO(yao): validate input */
        max_power = option_uint(&options->dbuf_max_power);
        max_size = buf_init_size << max_power;
    }

    dbuf_init = true;
}

void
dbuf_teardown(void)
{
    log_info("tear down the %s module", DBUF_MODULE_NAME);

    if (!dbuf_init) {
        log_warn("%s was not setup", DBUF_MODULE_NAME);
    }

    dbuf_init = false;
}

static rstatus_i
_dbuf_resize(struct buf **buf, uint32_t nsize)
{
    struct buf *nbuf;
    uint32_t osize, roffset, woffset;

    if (nsize > max_size) {
        return CC_ERROR;
    }

    osize = buf_size(*buf);
    roffset = (*buf)->rpos - (*buf)->begin;
    woffset = (*buf)->wpos - (*buf)->begin;

    nbuf = cc_realloc(*buf, nsize);
    if (nbuf == NULL) { /* realloc failed, but *buf is still valid */
        return CC_ENOMEM;
    }

    log_verb("buf %p of size %"PRIu32" resized to %p of size %"PRIu32, *buf,
            osize, nbuf, nsize);

    /* end, rpos, wpos need to be adjusted for the new address of buf */
    nbuf->end = (char *)nbuf + nsize;
    nbuf->rpos = nbuf->begin + roffset;
    nbuf->wpos = nbuf->begin + woffset;
    *buf = nbuf;
    DECR_N(buf_metrics, buf_memory, osize);
    INCR_N(buf_metrics, buf_memory, nsize);

    return CC_OK;
}

rstatus_i
dbuf_double(struct buf **buf)
{
    rstatus_i status;
    uint32_t nsize = buf_size(*buf) * 2;

    status = _dbuf_resize(buf, nsize);
    if (status == CC_OK) {
        INCR(dbuf_metrics, dbuf_double);
    } else {
        INCR(dbuf_metrics, dbuf_double_ex);
    }

    return status;
}

rstatus_i
dbuf_fit(struct buf **buf, uint32_t cap)
{
    rstatus_i status = CC_OK;
    uint32_t nsize = buf_init_size;

    /* check if new cap can contain unread bytes */
    if (buf_rsize(*buf) > cap) {
        return CC_ERROR;
    }

    buf_lshift(*buf);

    /* double size of buf until it can fit cap */
    while (nsize < cap + BUF_HDR_SIZE) {
        nsize *= 2;
    }

    if (nsize != buf_size(*buf)) {
        status = _dbuf_resize(buf, nsize);
        if (status == CC_OK) {
            INCR(dbuf_metrics, dbuf_fit);
        } else {
            INCR(dbuf_metrics, dbuf_fit_ex);
        }
    }

    return status;
}

rstatus_i
dbuf_shrink(struct buf **buf)
{
    uint32_t nsize = buf_init_size;
    uint32_t cap = buf_rsize(*buf);
    rstatus_i status = CC_OK;

    buf_lshift(*buf);

    while (nsize < cap + BUF_HDR_SIZE) {
        nsize *= 2;
    }

    if (nsize != buf_size(*buf)) {
        /*
         * realloc is not guaranteed to succeed even on trim, but in the case
         * that it fails, original buf will still be valid.
         */
        status = _dbuf_resize(buf, nsize);

        if (status == CC_OK) {
            INCR(dbuf_metrics, dbuf_shrink);
        } else {
            INCR(dbuf_metrics, dbuf_shrink_ex);
        }
    }

    return status;
}

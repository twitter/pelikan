#include "process.h"

#include "protocol/data/memcache_include.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>

#define REDIS_PROCESS_MODULE_NAME "redis::process"

#define OVERSIZE_ERR_MSG    "oversized value, cannot be stored"
#define DELTA_ERR_MSG       "value is not a number"
#define OOM_ERR_MSG         "server is out of memory"
#define CMD_ERR_MSG         "command not supported"
#define OTHER_ERR_MSG       "unknown server error"

typedef enum put_rstatus {
    PUT_OK,
    PUT_PARTIAL,
    PUT_ERROR,
} put_rstatus_t;

/* the data pointer in the process functions is of type `struct data **' */
struct data {
    struct request *req;
    struct response *rsp;
};

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;
static bool allow_flush = ALLOW_FLUSH;

void
process_setup(process_options_st *options, process_metrics_st *metrics)
{
    log_info("set up the %s module", REDIS_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 REDIS_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
    }

    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", REDIS_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", REDIS_PROCESS_MODULE_NAME);
    }

    allow_flush = false;
    process_metrics = NULL;
    process_init = false;
}

int
redis_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{

    return 0;
}


int
redis_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    dbuf_shrink(rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}


int
redis_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}

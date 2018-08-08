#include "process.h"

#include "protocol/admin/admin_include.h"
#include "util/procinfo.h"

#include <cc_mm.h>
#include <cc_print.h>

#define DS_ADMIN_MODULE_NAME "ds::admin"

extern struct stats stats;
extern unsigned int nmetric;

static bool admin_init = false;
static char *buf = NULL;
static size_t cap;

void
admin_process_setup(void)
{
    log_info("set up the %s module", DS_ADMIN_MODULE_NAME);
    if (admin_init) {
        log_warn("%s has already been setup, overwrite",
                 DS_ADMIN_MODULE_NAME);
    }

    cap = nmetric * METRIC_PRINT_LEN;
    buf = cc_alloc(cap);
    /* TODO: check return status of cc_alloc */

    admin_init = true;
}

void
admin_process_teardown(void)
{
    log_info("tear down the %s module", DS_ADMIN_MODULE_NAME);
    if (!admin_init) {
        log_warn("%s has never been setup", DS_ADMIN_MODULE_NAME);
    }

    admin_init = false;
}

static void
_admin_stats_default(struct response *rsp, struct request *req)
{
    procinfo_update();
    rsp->data.data = buf;
    rsp->data.len = print_stats(buf, cap, (struct metric *)&stats, nmetric);
}

static void
_admin_stats(struct response *rsp, struct request *req)
{
    if (bstring_empty(&req->arg)) {
        _admin_stats_default(rsp, req);
        return;
    } else {
        rsp->type = RSP_INVALID;
    }
}

void
admin_process_request(struct response *rsp, struct request *req)
{
    rsp->type = RSP_GENERIC;

    switch (req->type) {
    case REQ_STATS:
        _admin_stats(rsp, req);
        break;
    case REQ_VERSION:
        rsp->data = str2bstr(VERSION_PRINTED);
        break;
    default:
        rsp->type = RSP_INVALID;
        break;
    }
}

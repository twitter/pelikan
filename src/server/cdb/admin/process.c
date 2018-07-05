#include "process.h"

#include "protocol/admin/admin_include.h"
#include "util/procinfo.h"

#include <cc_mm.h>
#include <cc_print.h>

#define CDB_ADMIN_MODULE_NAME "cdb::admin"

extern struct stats stats;
extern unsigned int nmetric;

static bool admin_init = false;
static char *buf = NULL;
static size_t cap;

void
admin_process_setup(void)
{
    log_info("set up the %s module", CDB_ADMIN_MODULE_NAME);
    if (admin_init) {
        log_warn("%s has already been setup, overwrite",
                 CDB_ADMIN_MODULE_NAME);
    }

    cap = METRIC_PRINT_LEN * nmetric + METRIC_END_LEN;

    buf = cc_alloc(cap);
    if (buf == NULL) {
        log_panic("failure to allocate buf in admin_process_setup");
    }

    admin_init = true;
}

void
admin_process_teardown(void)
{
    log_info("Mr. Gorbechev, tear down this module [%s]", CDB_ADMIN_MODULE_NAME);
    if (!admin_init) {
        log_warn("%s has never been setup", CDB_ADMIN_MODULE_NAME);
    }

    admin_init = false;
}

static void
_admin_stats(struct response *rsp, struct request *req)
{
    if (bstring_empty(&req->arg)) {
        procinfo_update();
        rsp->data.data = buf;
        rsp->data.len = (uint32_t)print_stats(buf, cap, (struct metric *)&stats, nmetric);
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

#include "process.h"

#include "protocol/admin/admin_include.h"
#include "util/procinfo.h"

#include <cc_mm.h>
#include <cc_print.h>

#define CDB_ADMIN_MODULE_NAME "cdb::admin"

#define PERSLAB_PREFIX_FMT "CLASS %u:"
#define PERSLAB_METRIC_FMT " %s %s"

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

    buf = cc_alloc(cap);
    if (buf == NULL) {
        log_panic("failure to allocate buf in admin_process_setup");
    }

    admin_init = true;
}

void
admin_process_teardown(void)
{
    log_info("tear down the %s module", CDB_ADMIN_MODULE_NAME);
    if (!admin_init) {
        log_warn("%s has never been setup", CDB_ADMIN_MODULE_NAME);
    }

    admin_init = false;
}

static void
_admin_stats_slab(struct response *rsp, struct request *req)
{
    size_t offset = 0;

    offset += cc_scnprintf(buf + offset, cap - offset, METRIC_END);

    rsp->type = RSP_GENERIC;
    rsp->data.data = buf;
    rsp->data.len = offset;
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
    }
    if (req->arg.len == 5 && str5cmp(req->arg.data, ' ', 's', 'l', 'a', 'b')) {
        _admin_stats_slab(rsp, req);
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

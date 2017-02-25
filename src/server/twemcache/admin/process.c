#include "process.h"

#include "protocol/admin/admin_include.h"
#include "storage/slab/slab.h"
#include "util/procinfo.h"

#include <cc_mm.h>
#include <cc_print.h>

#define TWEMCACHE_ADMIN_MODULE_NAME "twemcache::admin"

#define PERSLAB_PREFIX_FMT "CLASS %u:"
#define PERSLAB_METRIC_FMT " %s %s"

extern struct stats stats;
extern unsigned int nmetric;
static unsigned int nmetric_perslab = METRIC_CARDINALITY(perslab_metrics_st);

static bool admin_init = false;
static char *buf = NULL;
static size_t cap;

void
admin_process_setup(void)
{
    log_info("set up the %s module", TWEMCACHE_ADMIN_MODULE_NAME);
    if (admin_init) {
        log_warn("%s has already been setup, overwrite",
                 TWEMCACHE_ADMIN_MODULE_NAME);
    }

    nmetric_perslab = METRIC_CARDINALITY(perslab[0]);
    /* perslab metric size <(32 + 20)B, prefix/suffix 12B, total < 64 */
    cap = MAX(nmetric, nmetric_perslab * SLABCLASS_MAX_ID) * METRIC_PRINT_LEN;
    buf = cc_alloc(cap);
    /* TODO: check return status of cc_alloc */

    admin_init = true;
}

void
admin_process_teardown(void)
{
    log_info("tear down the %s module", TWEMCACHE_ADMIN_MODULE_NAME);
    if (!admin_init) {
        log_warn("%s has never been setup", TWEMCACHE_ADMIN_MODULE_NAME);
    }

    admin_init = false;
}

static void
_admin_stats_slab(struct response *rsp, struct request *req)
{
    uint8_t id;
    size_t offset = 0;

    for (id = SLABCLASS_MIN_ID; id <= profile_last_id; id++) {
        struct metric *metrics = (struct metric *)&perslab[id];
        offset += cc_scnprintf(buf + offset, cap - offset,
                PERSLAB_PREFIX_FMT, id);
        for (int i = 0; i < nmetric_perslab; i++) {
            offset += metric_print(buf + offset, cap - offset,
                   PERSLAB_METRIC_FMT, &metrics[i]);
        }
        offset += cc_scnprintf(buf + offset, cap - offset, CRLF);
    }
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

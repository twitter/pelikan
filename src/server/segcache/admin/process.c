#include "process.h"

#include "protocol/admin/admin_include.h"
#include "util/procinfo.h"

#include <cc_mm.h>
#include <cc_print.h>
#include <cc_stats_log.h>

#include <storage/seg/ttlbucket.h>
#include <sysexits.h>
#include <time/time.h>

#define SEGCACHE_ADMIN_MODULE_NAME "segcache::admin"

#define PERTTL_PREFIX_FMT "TTL_BUCKET (ttl %u):"
#define PERTTL_METRIC_FMT " %s %s"

extern struct stats stats;
extern unsigned int nmetric;
extern seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];
static unsigned int nmetric_perttl = METRIC_CARDINALITY(seg_perttl_metrics_st);
extern struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];


static bool admin_init = false;
static char *buf = NULL;
static size_t cap;

void
admin_process_setup(void)
{
    log_info("set up the %s module", SEGCACHE_ADMIN_MODULE_NAME);
    if (admin_init) {
        log_warn("%s has already been setup, overwrite",
                SEGCACHE_ADMIN_MODULE_NAME);
    }

    nmetric_perttl = METRIC_CARDINALITY(perttl[0]);
    cap = MAX(nmetric, nmetric_perttl * MAX_TTL_BUCKET) * METRIC_PRINT_LEN +
            METRIC_END_LEN;
    buf = cc_alloc(cap);
    if (buf == NULL) {
        log_crit("cannot allocate buffer for admin stat string");
        exit(EX_OSERR);
    }
    admin_init = true;
}

void
admin_process_teardown(void)
{
    log_info("tear down the %s module", SEGCACHE_ADMIN_MODULE_NAME);
    if (!admin_init) {
        log_warn("%s has never been setup", SEGCACHE_ADMIN_MODULE_NAME);
    }

    /* TODO (jason) free buf */
    cc_free(buf);
    admin_init = false;
}

static void
_admin_stats_ttl(struct response *rsp, struct request *req)
{
    uint32_t idx;
    size_t offset = 0;

    for (idx = 0; idx < MAX_TTL_BUCKET; idx++) {
        struct ttl_bucket *ttl_bucket = &ttl_buckets[idx];
        if (ttl_bucket->n_seg == 0) {
            /* do not print empty ttl bucket */
            continue;
        }

        struct metric *metrics = (struct metric *)&perttl[idx];
        offset += cc_scnprintf(buf + offset, cap - offset,
                PERTTL_PREFIX_FMT, ttl_bucket->ttl);
        for (int i = 0; i < nmetric_perttl; i++) {
            offset += metric_print(buf + offset, cap - offset,
                   PERTTL_METRIC_FMT, &metrics[i]);
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
    if (req->arg.len == 4 && str4cmp(req->arg.data, ' ', 's', 'e', 'g')) {
        _admin_stats_ttl(rsp, req);
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

void
stats_dump(void *arg)
{
    procinfo_update();
    stats_log((struct metric *)&stats, nmetric);
    stats_log_flush();
}

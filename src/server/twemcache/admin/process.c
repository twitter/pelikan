#include "process.h"

#include "protocol/admin/admin_include.h"
#include "storage/slab/slab.h"
#include "util/procinfo.h"

#include <cc_mm.h>
#include <cc_print.h>

#define TWEMCACHE_ADMIN_MODULE_NAME "twemcache::admin"

#define METRIC_PRINT_FMT "STAT %s %s\r\n"
#define METRIC_PRINT_LEN 64 /* > 5("STAT ") + 32 (name) + 20 (value) + CRLF */
#define METRIC_DESCRIBE_FMT "%33s %15s %s\r\n"
#define METRIC_DESCRIBE_LEN 120 /* 34 (name) + 16 (type) + 68 (description) + CRLF */
#define METRIC_END "END\r\n"
#define METRIC_END_LEN (sizeof(METRIC_END) - 1)

#define PERSLAB_PREFIX_FMT "CLASS %u:"
#define PERSLAB_METRIC_FMT " %s %s"
#define PERSLAB_SUFFIX_FMT "\r\n"

#define VERSION_PRINT_FMT "VERSION %s\r\n"
#define VERSION_PRINT_LEN 30

extern struct stats stats;
extern unsigned int nmetric;

static bool admin_init = false;
static admin_process_metrics_st *admin_metrics = NULL;
static char *stats_buf = NULL;
static char version_buf[VERSION_PRINT_LEN];
static size_t stats_len;
static unsigned int nmetric_perslab;

void
admin_process_setup(admin_process_metrics_st *metrics)
{
    log_info("set up the %s module", TWEMCACHE_ADMIN_MODULE_NAME);
    if (admin_init) {
        log_warn("%s has already been setup, overwrite",
                 TWEMCACHE_ADMIN_MODULE_NAME);
    }

    admin_metrics = metrics;

    nmetric_perslab = METRIC_CARDINALITY(perslab[0]);
    /* perslab metric size <(32 + 20)B, prefix/suffix 12B, total < 64 */
    stats_len = MAX(nmetric, nmetric_perslab * SLABCLASS_MAX_ID) *
        METRIC_PRINT_LEN;
    stats_buf = cc_alloc(stats_len + METRIC_END_LEN);
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

    admin_metrics = NULL;
    admin_init = false;
}

static void
_admin_stats_slab(struct response *rsp, struct request *req)
{
    uint8_t id;
    size_t offset = 0;

    for (id = SLABCLASS_MIN_ID; id <= profile_last_id; id++) {
        struct metric *metrics = (struct metric *)&perslab[id];
        offset += cc_scnprintf(stats_buf + offset, stats_len - offset,
                PERSLAB_PREFIX_FMT, id);
        for (int i = 0; i < nmetric_perslab; i++) {
            offset += metric_print(stats_buf + offset, stats_len - offset,
                   PERSLAB_METRIC_FMT, &metrics[i]);
        }
        offset += cc_scnprintf(stats_buf + offset, stats_len - offset,
                PERSLAB_SUFFIX_FMT);
    }
    offset += cc_scnprintf(stats_buf + offset, stats_len - offset, METRIC_END);

    rsp->type = RSP_GENERIC;
    rsp->data.data = stats_buf;
    rsp->data.len = offset;
}

static void
_admin_stats_default(struct response *rsp, struct request *req)
{
    size_t offset = 0;
    struct metric *metrics = (struct metric *)&stats;

    INCR(admin_metrics, stats);

    procinfo_update();
    for (int i = 0; i < nmetric; ++i) {
        offset += metric_print(stats_buf + offset, stats_len - offset,
                METRIC_PRINT_FMT, &metrics[i]);
    }
    offset += cc_scnprintf(stats_buf + offset, stats_len - offset, METRIC_END);

    rsp->type = RSP_GENERIC;
    rsp->data.data = stats_buf;
    rsp->data.len = offset;
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

static void
_admin_version(struct response *rsp, struct request *req)
{
    INCR(admin_metrics, version);

    rsp->type = RSP_GENERIC;
    cc_snprintf(version_buf, VERSION_PRINT_LEN, VERSION_PRINT_FMT, VERSION_STRING);
    rsp->data = str2bstr(version_buf);
}

void
admin_process_request(struct response *rsp, struct request *req)
{
    switch (req->type) {
    case REQ_STATS:
        _admin_stats(rsp, req);
        break;
    case REQ_VERSION:
        _admin_version(rsp, req);
        break;
    default:
        rsp->type = RSP_INVALID;
        break;
    }
}

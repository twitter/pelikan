#include <twemcache/admin/process.h>

#include <protocol/admin/admin_include.h>
#include <util/stats.h>
#include <util/procinfo.h>

#include <cc_mm.h>
#include <cc_print.h>

#define SLIMCACHE_ADMIN_MODULE_NAME "slimcache::admin"

#define METRIC_HEADER "STAT "
#define METRIC_PRINT_FMT "%s %s\r\n"
#define METRIC_PRINT_LEN 64 /* 32 (name) + 30 (value) + CRLF */
#define METRIC_DESCRIBE_FMT "%33s %15s %s\r\n"
#define METRIC_DESCRIBE_LEN 120 /* 34 (name) + 16 (type) + 68 (description) + CRLF */
#define METRIC_FOOTER CRLF
#define METRIC_END "END\r\n"
#define METRIC_END_LEN sizeof(METRIC_END)

#define VERSION_PRINT_FMT "VERSION %s\r\n"
#define VERSION_PRINT_LEN 30

static bool admin_init = false;
static admin_process_metrics_st *admin_metrics = NULL;
static char *stats_buf = NULL;
static char version_buf[VERSION_PRINT_LEN];
static size_t card, stats_len;

void
admin_process_setup(admin_process_metrics_st *metrics)
{
    log_info("set up the %s module", SLIMCACHE_ADMIN_MODULE_NAME);
    if (admin_init) {
        log_warn("%s has already been setup, overwrite",
                 SLIMCACHE_ADMIN_MODULE_NAME);
    }

    card = stats_card();
    stats_len = METRIC_PRINT_LEN * card;
    stats_buf = cc_alloc(stats_len + METRIC_END_LEN);
    /* TODO: check return status of cc_alloc */

    admin_metrics = metrics;
    ADMIN_PROCESS_METRIC_INIT(admin_metrics);
    admin_init = true;
}

void
admin_process_teardown(void)
{
    log_info("tear down the %s module", SLIMCACHE_ADMIN_MODULE_NAME);
    if (!admin_init) {
        log_warn("%s has never been setup", SLIMCACHE_ADMIN_MODULE_NAME);
    }

    admin_metrics = NULL;
    admin_init = false;
}

static void
_admin_stats(struct response *rsp, struct request *req)
{
    size_t offset = 0;

    INCR(admin_metrics, stats);

    procinfo_update();
    for (int i = 0; i < card; ++i) {
        offset += metric_print(stats_buf + offset, stats_len - offset,
                METRIC_PRINT_FMT, &gs[i]);
    }
    strcpy(stats_buf + offset, METRIC_END);

    rsp->type = RSP_GENERIC;
    rsp->data.data = stats_buf;
    rsp->data.len = offset + METRIC_END_LEN;
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

#include <protocol/memcache/klog.h>

#include <protocol/memcache/request.h>
#include <protocol/memcache/response.h>
#include <time/time.h>
#include <util/log_core.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_log.h>
#include <cc_print.h>

#include <errno.h>
#include <stdbool.h>
#include <time.h>

#define KLOG_MODULE_NAME   "protocol::memcache:klog"
#define KLOG_MAX_LEN       KiB

/* TODO(yao): Use a cheaper way to format the command logs, e.g. print_uint64 */
/* TODO(yao): timestamp can be optimized by not reformatting within a second */
#define KLOG_TIME_FMT      "[%d/%b/%Y:%T %z] "
#define KLOG_STORE_FMT     "\"%.*s%.*s %u %u %u\" %d %u\n"
#define KLOG_CAS_FMT       "\"%.*s%.*s %u %u %u %llu\" %d %u\n"
#define KLOG_GET_FMT       "\"%.*s %.*s\" %d %u\n"
#define KLOG_DELTA_FMT     "\"%.*s%.*s %llu\" %d %u\n"

static bool klog_init = false;
static struct logger *klogger = NULL;
struct log_core *klog_core = NULL;
static uint64_t klog_cmds = 0;
static uint32_t klog_sample = KLOG_SAMPLE;
static klog_metrics_st *klog_metrics = NULL;

rstatus_t
klog_setup(char *file, uint32_t nbuf, uint32_t interval, uint32_t sample, klog_metrics_st *metrics)
{
    log_info("Set up the %s module", KLOG_MODULE_NAME);

    if (klog_init) {
        log_warn("%s has already been setup, overwrite", KLOG_MODULE_NAME);
    }

    klog_metrics = metrics;
    if (metrics != NULL) {
        KLOG_METRIC_INIT(klog_metrics);
    }

    if (klogger != NULL) {
        log_destroy(&klogger);
    }

    klogger = log_create(file, nbuf);

    if (klogger == NULL) {
        log_error("Could not create klogger!");
        return CC_ERROR;
    }

    klog_core = log_core_create(klogger, interval);

    if (klog_core == NULL) {
        log_error("Could not create klog core");
        log_destroy(&klogger);
        return CC_ERROR;
    }

    if (klog_sample == 0) {
        log_error("klog sample rate cannot be 0 - divide by zero");
        log_destroy(&klogger);
        return CC_ERROR;
    }
    klog_sample = sample;

    klog_init = true;

    return CC_OK;
}

void
klog_teardown(void)
{
    log_info("Tear down the %s module", KLOG_MODULE_NAME);

    if (!klog_init) {
        log_warn("%s was not setup", KLOG_MODULE_NAME);
    }

    klog_metrics = NULL;
    log_core_destroy(&klog_core);

    if (klogger != NULL) {
        log_destroy(&klogger);
    }

    klog_sample = 1;

    klog_init = false;
}

/* TODO(kyang): add accurate size or upper-bound of seralized req/rsp objects (CACHE-3482) */
static inline uint32_t
_get_val_rsp_len(struct response *rsp, struct bstring *key)
{
    /* rsp = rsp string + key + " " + flag + " " + vlen (+ " " + cas)(if gets) + crlf + val + crlf */
    return rsp_strings[rsp->type].len + key->len + 1 + digits(rsp->flag) + 1
        + digits(rsp->vstr.len) + (rsp->cas ? 1 + digits(rsp->vcas) : 0) + CRLF_LEN
        + (rsp->num ? digits(rsp->vint) : rsp->vstr.len) + CRLF_LEN;
}

static inline void
_klog_write_get(struct request *req, struct response *rsp, char *buf, int len)
{
    struct response *nr = rsp;
    int suffix_len;
    uint32_t i;
    struct bstring *key;

    for (i = 0; i < array_nelem(req->keys); ++i) {
        key = array_get(req->keys, i);

        if (nr->type != RSP_END && bstring_compare(key, &nr->key) == 0) {
            /* key was found, rsp at nr */
            suffix_len = cc_scnprintf(buf + len, KLOG_MAX_LEN - len, KLOG_GET_FMT,
                                      req_strings[req->type].len, req_strings[req->type].data,
                                      key->len, key->data, rsp->type, _get_val_rsp_len(nr, key));
            nr = STAILQ_NEXT(nr, next);
        } else {
            /* key not found */
            suffix_len = cc_scnprintf(buf + len, KLOG_MAX_LEN - len, KLOG_GET_FMT,
                                      req_strings[req->type].len, req_strings[req->type].data,
                                      key->len, key->data, RSP_UNKNOWN, 0);
        }

        ASSERT(len + suffix_len <= KLOG_MAX_LEN);

        if (_log_write(klogger, buf, len + suffix_len)) {
            INCR(klog_metrics, klog_logged);
        } else {
            INCR(klog_metrics, klog_discard);
        }
    }

    ASSERT(nr ->type == RSP_END);
}

static inline int
_klog_fmt_delete(struct request *req, struct response *rsp, char *buf, int len)
{
    struct bstring *key = array_get(req->keys, 0);

    len += cc_scnprintf(buf + len, KLOG_MAX_LEN - len, KLOG_GET_FMT, req_strings[req->type].len,
                        req_strings[req->type].data, key->len, key->data, rsp->type,
                        req->noreply ? 0 : rsp_strings[rsp->type].len);

    return len;
}

static inline int
_klog_fmt_store(struct request *req, struct response *rsp, char *buf, int len)
{
    struct bstring *key = array_get(req->keys, 0);

    len += cc_scnprintf(buf + len, KLOG_MAX_LEN - len, KLOG_STORE_FMT, req_strings[req->type].len,
                        req_strings[req->type].data, key->len, key->data, req->flag,
                        req->expiry, req->vstr.len, rsp->type,
                        req->noreply ? 0 : rsp_strings[rsp->type].len);

    return len;
}

static inline int
_klog_fmt_cas(struct request *req, struct response *rsp, char *buf, int len)
{
    struct bstring *key = array_get(req->keys, 0);

    len += cc_scnprintf(buf + len, KLOG_MAX_LEN - len, KLOG_CAS_FMT, req_strings[req->type].len,
                        req_strings[req->type].data, key->len, key->data, req->flag,
                        req->expiry, req->vstr.len, req->vcas, rsp->type,
                        req->noreply ? 0 : rsp_strings[rsp->type].len);

    return len;
}

static inline int
_klog_fmt_delta(struct request *req, struct response *rsp, char *buf, int len)
{
    uint32_t rsp_len;
    struct bstring *key = array_get(req->keys, 0);

    if (req->noreply) {
        rsp_len = 0;
    } else if (rsp->type == RSP_NUMERIC) {
        rsp_len = digits(rsp->vint) + CRLF_LEN;
    } else {
        rsp_len = rsp_strings[rsp->type].len;
    }

    len += cc_scnprintf(buf + len, KLOG_MAX_LEN - len, KLOG_DELTA_FMT, req_strings[req->type].len,
                        req_strings[req->type].data, key->len, key->data, req->delta,
                        rsp->type, rsp_len);

    return len;
}

/* TODO(kyang): update peer to log the peer instead of placeholder (CACHE-3492) */
void
klog_write(struct request *req, struct response *rsp)
{
    int len, time_len, errno_save;
    char buf[KLOG_MAX_LEN], *peer = "-";
    time_t t;

    if (klogger == NULL) {
        return;
    }

    ++klog_cmds;

    if (klog_cmds % klog_sample != 0) {
        INCR(klog_metrics, klog_skip);
        return;
    }

    errno_save = errno;

    t = time_now_abs();
    len = cc_scnprintf(buf, KLOG_MAX_LEN, "%s - ", peer);
    time_len = strftime(buf + len, KLOG_MAX_LEN - len, KLOG_TIME_FMT, localtime(&t));
    if (time_len == 0) {
        log_error("strftime failed: %s", strerror(errno));
        goto done;
    }
    len += time_len;

    switch (req->type) {
    case REQ_GET:
    case REQ_GETS:
        _klog_write_get(req, rsp, buf, len);
        goto done;
        break;
    case REQ_DELETE:
        len = _klog_fmt_delete(req, rsp, buf, len);
        break;
    case REQ_SET:
    case REQ_ADD:
    case REQ_REPLACE:
    case REQ_APPEND:
    case REQ_PREPEND:
        len = _klog_fmt_store(req, rsp, buf, len);
        break;
    case REQ_CAS:
        len = _klog_fmt_cas(req, rsp, buf, len);
        break;
    case REQ_INCR:
    case REQ_DECR:
        len = _klog_fmt_delta(req, rsp, buf, len);
        break;
    default:
        goto done;
    }

    ASSERT(len <= KLOG_MAX_LEN);

    if (_log_write(klogger, buf, len)) {
        INCR(klog_metrics, klog_logged);
    } else {
        INCR(klog_metrics, klog_discard);
    }

done:
    errno = errno_save;
}

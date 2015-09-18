#include <protocol/memcache/klog.h>

#include <protocol/memcache/request.h>
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
#define KLOG_STORE_FMT     "\"%s%.*s %u %u %u\" %d %u\n"
#define KLOG_CAS_FMT       "\"%s%.*s %u %u %u %llu\" %d %u\n"
#define KLOG_GET_FMT       "\"%s%.*s\" %d %u\n"
#define KLOG_DELTA_FMT     "\"%s%.*s %llu\" %d %u\n"

static bool klog_init = false;
static struct logger *klogger = NULL;
static struct log_core *klog_core = NULL;

rstatus_t
klog_setup(char *file, uint32_t nbuf, uint32_t interval)
{
    log_info("Set up the %s module", KLOG_MODULE_NAME);

    if (klog_init) {
        log_warn("%s has already been setup, overwrite", KLOG_MODULE_NAME);
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

    log_core_destroy(&klog_core);

    if (klogger != NULL) {
        log_destroy(&klogger);
    }

    klog_init = false;
}

static inline int
klog_fmt_get(char *buf, int size, struct request *req, int status, uint32_t rsp_len)
{
    struct bstring *key = req->keys->data;

    return cc_scnprintf(buf, size, KLOG_GET_FMT, req_strings[req->type],
                        key->len, key->data, status, rsp_len);
}

static inline int
klog_fmt_store(char *buf, int size, struct request *req, int status, uint32_t rsp_len)
{
    struct bstring *key = req->keys->data;

    return cc_scnprintf(buf, size, KLOG_STORE_FMT, req_strings[req->type],
                        key->len, key->data, req->flag, req->expiry, req->vstr.len,
                        status, rsp_len);
}

static inline int
klog_fmt_cas(char *buf, int size, struct request *req, int status, uint32_t rsp_len)
{
    struct bstring *key = req->keys->data;

    return cc_scnprintf(buf, size, KLOG_CAS_FMT, req_strings[req->type],
                        key->len, key->data, req->flag, req->expiry, req->vstr.len,
                        req->vcas, status, rsp_len);
}

static inline int
klog_fmt_delta(char *buf, int size, struct request *req, int status, uint32_t rsp_len)
{
    struct bstring *key = req->keys->data;

    return cc_scnprintf(buf, size, KLOG_DELTA_FMT, req_strings[req->type],
                        key->len, key->data, req->delta, status, rsp_len);
}

static int
klog_fmt(char *buf, int size, struct request *req, int status, uint32_t rsp_len)
{
    switch (req->type) {
    case REQ_GET:
    case REQ_GETS:
    case REQ_DELETE:
        return klog_fmt_get(buf, size, req, status, rsp_len);
    case REQ_SET:
    case REQ_ADD:
    case REQ_REPLACE:
    case REQ_APPEND:
    case REQ_PREPEND:
        return klog_fmt_store(buf, size, req, status, rsp_len);
    case REQ_CAS:
        return klog_fmt_cas(buf, size, req, status, rsp_len);
    case REQ_INCR:
    case REQ_DECR:
        return klog_fmt_delta(buf, size, req, status, rsp_len);
    default:
        return 0;
    }
}

void
klog_write(struct request *req, int status, uint32_t rsp_len)
{
    int len, time_len, errno_save, size;
    char buf[KLOG_MAX_LEN], *peer = "-";
    time_t t;

    if (klogger == NULL) {
        return;
    }

    errno_save = errno;

    size = LOG_MAX_LEN; /* size of output buffer */

    t = time_now_abs();

    len = cc_scnprintf(buf, size, "%s - ", peer);

    time_len = strftime(buf + len, size - len, KLOG_TIME_FMT, localtime(&t));

    if (time_len == 0) {
        log_error("strftime failed: %s", strerror(errno));
        return;
    }

    len += time_len;

    len += klog_fmt(buf + len, size - len, req, status, rsp_len);

    _log_write(klogger, buf, len);

    errno = errno_save;
}

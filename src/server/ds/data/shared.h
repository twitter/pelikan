#include "protocol/data/resp_include.h"

/*TODO(yao): moving this to protocol/data/resp ? */

/* help functions to compose common responses */
static inline void
compose_rsp_ok(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_OK);
    log_verb("command '%.*s' '%.*s' succeeded", cmd->bstr.len, cmd->bstr.data,
            key->len, key->data);
}

static inline void
compose_rsp_oob(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key, int64_t idx)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_OUTOFRANGE);
    log_verb("command '%.*s' '%.*s' has idx %lld out of bounds",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data, idx);
}

static inline void
compose_rsp_notfound(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_NOTFOUND);
    log_verb("command '%.*s' '%.*s' miss, key not found",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
compose_rsp_nil(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_NIL;
    log_verb("command '%.*s' '%.*s' returns nil",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
compose_rsp_noop(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_NOOP);
    log_verb("command '%.*s' '%.*s' is a noop",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
compose_rsp_client_err(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_ARG);
    INCR(process_metrics, process_client_ex);
    log_debug("command '%.*s' '%.*s' has invalid arg(s)",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
compose_rsp_storage_err(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_STORAGE);
    INCR(process_metrics, process_server_ex);
    log_warn("command '%.*s' '%.*s' failed, unable to allocate storage",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
compose_rsp_server_err(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_SERVER);
    INCR(process_metrics, process_server_ex);
    log_warn("command '%.*s' '%.*s' failed, unspecified server error",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

/* helper functions for getting elements from the request */
static inline bool
req_get_bstr(struct bstring **bstr, const struct request *req, uint32_t offset)
{
    ASSERT(array_nelem(req->token) > offset);
    ASSERT(bstr != NULL);

    struct element *e = (struct element *)array_get(req->token, offset);

    if (e->type != ELEM_BULK) {
        return false;
    }

    *bstr = &e->bstr;
    return true;
}

static inline bool
req_get_int(int64_t *i, const struct request *req, uint32_t offset)
{
    ASSERT(array_nelem(req->token) > offset);
    ASSERT(i != NULL);

    struct element *e = (struct element *)array_get(req->token, offset);
    log_verb("check type, %u", e->type);
    if (e->type != ELEM_INT) {
        return false;
    }

    *i = e->num;
    return true;
}

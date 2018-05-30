#include "process.h"

#include "data_structure/ziplist/ziplist.h"
#include "protocol/data/redis_include.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>


static inline struct bstring *
_get_key(struct request *req)
{
    struct element *key = (struct element *)array_get(req->token, LIST_KEY);

    return &key->bstr;
}

static inline struct item *
_add_key(struct response *rsp, struct bstring *key)
{
    struct element *reply = (struct element *)array_get(rsp->token, 0);
    struct item *it;
    item_rstatus_t istatus;

    it = item_get(key);
    if (it != NULL) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, list_create_exist);

        return NULL;
    } else {
        /* TODO: figure out a TTL story here */
        istatus = item_reserve(&it, key, NULL, ZIPLIST_HEADER_SIZE, 0, UINT32_MAX);
        if (istatus != ITEM_OK) {
            rsp->type = reply->type = ELEM_ERR;
            reply->bstr = str2bstr(RSP_ERR_STORAGE);
            INCR(process_metrics, list_create_ex);
            INCR(process_metrics, process_ex);
        } else {
            INCR(process_metrics, list_create_stored);
        }

        return it;
    }
}


void
cmd_list_create(struct response *rsp, struct request *req, struct command *cmd)
{
    struct item *it;
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);

    INCR(process_metrics, list_create);

    it = _add_key(rsp, key);
    if (it == NULL) {
        log_debug("command '%.*s' '%.*s' failed: cannot store", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        return;
    }

    /* initialize data structure */
    ziplist_reset((ziplist_p)item_data(it));
    it->vlen = ZIPLIST_HEADER_SIZE;

    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_OK);

    log_verb("command '%.*s' '%.*s' succeeded", cmd->bstr.len, cmd->bstr.data,
            key->len, key->data);
}

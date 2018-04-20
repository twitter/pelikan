#include "process.h"

#include "protocol/data/redis_include.h"
#include "storage/cuckoo/cuckoo.h"
#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>


static inline struct bstring
_get_key(struct request *req)
{
    struct element *key = (struct element *)array_get(req->token, 1);

    return key->bstr;
}

void
cmd_bitmap_delete(struct response *rsp, struct request *req, struct command *cmd)
{
    struct element *el;
    struct bstring key;
    bool status;

    INCR(process_metrics, bitmap_delete);

    key = _get_key(req);

    rsp->type = ELEM_STR;
    el = array_push(rsp->token);
    ASSERT(el != NULL); /* cannot fail because we preallocate tokens */
    el->type = ELEM_STR;
    status = cuckoo_delete(&key);
    if (status) {
        el->bstr = str2bstr(RSP_DELETED);
        INCR(process_metrics, bitmap_delete_deleted);
    } else {
        el->bstr = str2bstr(RSP_NOT_FOUND);
        INCR(process_metrics, bitmap_delete_notfound);
    }

    log_verb("command '%.*s' on key '%.*s' returned %d", cmd->bstr.len,
            cmd->bstr.data, key.len, key.data, status);
}

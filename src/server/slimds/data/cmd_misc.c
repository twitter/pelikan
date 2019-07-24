#include "process.h"

#include "protocol/data/resp_include.h"
#include <cc_array.h>
#include <cc_debug.h>


bool allow_flush = ALLOW_FLUSH;

void
cmd_ping(struct response *rsp, struct request *req, struct command *cmd)
{
    struct element *el = NULL;

    el = array_push(rsp->token);
    ASSERT(el != NULL); /* cannot fail because we preallocate tokens */

    if (cmd->nopt == 0) { /* no additional argument, respond pong */
        rsp->type = ELEM_STR;
        el->type = ELEM_STR;
        el->bstr = str2bstr(RSP_PONG);
    } else { /* behave as echo, use bulk string */
        struct element *arg = (struct element *)array_get(req->token, 1);
        rsp->type = ELEM_BULK;
        el->type = ELEM_BULK;
        el->bstr = arg->bstr;
    }

    INCR(process_metrics, ping);
}

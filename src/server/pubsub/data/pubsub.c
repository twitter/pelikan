#include "pubsub.h"

#include "protocol/data/redis_include.h"
#include "storage/pubsub/listener.h"
#include "storage/pubsub/topic.h"

#include <cc_array.h>
#include <stream/cc_sockio.h>

static struct listener_ht *lht;
static struct topic_ht *tht;

void
pubsub_setup(void)
{
    lht = listener_ht_create(16);
    tht = topic_ht_create(16);
}

void
pubsub_teardown(void)
{
    listener_ht_destroy(&lht);
    topic_ht_destroy(&tht);
}

/* "subscribe topic [topic ...]" */
void
command_subscribe(struct response *rsp, struct request *req, struct buf_sock *s)
{
    struct element *el;
    struct listener *l;
    struct topic *t;
    uint32_t ntopic = req->token->nelem - 1;

    l = listener_ht_get(s->ch, lht);
    if (l == NULL) {
        l = listener_create(s->ch, s->hdl);
        listener_ht_put(l, lht);
    }

    for (int i = 1; i < ntopic; i++) {
        el = array_get(req->token, i);
        if (el->type != ELEM_BULK) {
            /* handle client error */
        };

        t = topic_ht_get(&el->bstr, tht);
        if (t == NULL) {
            t = topic_create(&el->bstr);
        }
        listener_add_topic(l, t);
    }

    rsp->type = ELEM_STR;
    el = array_push(rsp->token);
    el->type = ELEM_STR;
    el->bstr = str2bstr(RSP_STR_OK);
}

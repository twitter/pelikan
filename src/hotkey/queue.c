#include "queue.h"
#include "constant.h"

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define QUEUE_MODULE_NAME "hotkey::queue"

static bool queue_init = false;
static int queue_size = 0;

FREEPOOL(qn_pool, qnq, queue_node);
static struct qn_pool qnp;
static bool qnp_init = false;

struct queue_node {
    char                     key[MAX_KEY_LEN];
    uint32_t                 nkey;
    STAILQ_ENTRY(queue_node) next;
};

STAILQ_HEAD(queue, queue_node);
struct queue q = STAILQ_HEAD_INITIALIZER(q);

static void
queue_node_reset(struct queue_node *qn)
{
    qn->nkey = 0;
}

static struct queue_node *
queue_node_create(void)
{
    struct queue_node *qn = cc_alloc(sizeof(*qn));

    if (qn == NULL) {
        return NULL;
    }

    queue_node_reset(qn);

    return qn;
}

static void
queue_node_destroy(struct queue_node **queue_node)
{
    struct queue_node *qn = *queue_node;
    ASSERT(qn != NULL);

    cc_free(qn);
    *queue_node = NULL;
}

static void
queue_node_pool_destroy(void)
{
    struct queue_node *qn, *tqn;

    if (!qnp_init) {
        log_warn("queue_node pool was not created, ignore");
    }

    log_info("destroying queue_node pool: free %"PRIu32, qnp.nfree);

    FREEPOOL_DESTROY(qn, tqn, &qnp, next, queue_node_destroy);
    qnp_init = false;
}

static void
queue_node_pool_create(uint32_t max)
{
    struct queue_node *qn;

    if (qnp_init) {
        log_warn("queue_node pool has already been created, re-creating");
        queue_node_pool_destroy();
    }

    log_info("creating queue_node pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&qnp, max);
    qnp_init = true;

    FREEPOOL_PREALLOC(qn, &qnp, max, next, queue_node_create);
    if (qnp.nfree < max) {
        log_crit("cannot preallocate queue_node pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

static struct queue_node *
queue_node_borrow(void)
{
    struct queue_node *qn;

    FREEPOOL_BORROW(qn, &qnp, next, queue_node_create);
    if (qn == NULL) {
        log_debug("borrow queue_node failed: OOM");
        return NULL;
    }
    queue_node_reset(qn);

    return qn;
}

static void
queue_node_return(struct queue_node **queue_node)
{
    struct queue_node *qn = *queue_node;

    if (qn == NULL) {
        return;
    }

    FREEPOOL_RETURN(qn, &qnp, next);

    *queue_node = NULL;
}

void
queue_setup(uint32_t poolsize)
{
    log_info("set up the %s module", QUEUE_MODULE_NAME);

    if (queue_init) {
        log_warn("%s has already been setup, overwrite", QUEUE_MODULE_NAME);
    }

    queue_node_pool_create(poolsize);
    queue_size = 0;
    STAILQ_INIT(&q);
    queue_init = true;
}

void
queue_teardown(void)
{
    struct queue_node *qn, *tqn;

    log_info("tear down the %s module", QUEUE_MODULE_NAME);

    if (!queue_init) {
        log_warn("%s was not setup", QUEUE_MODULE_NAME);
    }

    /* free all entries in queue */
    STAILQ_FOREACH_SAFE(qn, &q, next, tqn) {
        queue_node_return(&qn);
    }

    queue_node_pool_destroy();
    queue_init = false;
}

void
queue_push(char *key, uint32_t nkey)
{
    struct queue_node *qn = queue_node_borrow();

    ASSERT(nkey <= MAX_KEY_LEN);

    cc_memcpy(qn->key, key, nkey);
    qn->nkey = nkey;
    STAILQ_INSERT_TAIL(&q, qn, next);
    ++queue_size;
}

uint32_t
queue_pop(char *buf)
{
    struct queue_node *qn = STAILQ_FIRST(&q);
    uint32_t nkey;

    cc_memcpy(buf, qn->key, qn->nkey);
    nkey = qn->nkey;

    STAILQ_REMOVE_HEAD(&q, next);
    queue_node_return(&qn);
    --queue_size;

    return nkey;
}

uint32_t
queue_len(void)
{
    return queue_size;
}

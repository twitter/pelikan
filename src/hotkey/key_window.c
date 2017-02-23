#include "key_window.h"
#include "constant.h"

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define KEY_WINDOW_MODULE_NAME "hotkey::key_window"

static bool key_window_init = false;
static int key_window_size = 0;

FREEPOOL(kwn_pool, kwnq, key_window_node);
static struct kwn_pool kwnp;
static bool kwnp_init = false;

struct key_window_node {
    char                     key[MAX_KEY_LEN];
    uint32_t                 nkey;
    STAILQ_ENTRY(key_window_node) next;
};

STAILQ_HEAD(key_window, key_window_node);
struct key_window q = STAILQ_HEAD_INITIALIZER(q);

static void
key_window_node_reset(struct key_window_node *kwn)
{
    kwn->nkey = 0;
}

static struct key_window_node *
key_window_node_create(void)
{
    struct key_window_node *kwn = cc_alloc(sizeof(*kwn));

    if (kwn == NULL) {
        return NULL;
    }

    key_window_node_reset(kwn);

    return kwn;
}

static void
key_window_node_destroy(struct key_window_node **key_window_node)
{
    struct key_window_node *kwn = *key_window_node;
    ASSERT(kwn != NULL);

    cc_free(kwn);
    *key_window_node = NULL;
}

static void
key_window_node_pool_destroy(void)
{
    struct key_window_node *kwn, *tkwn;

    if (!kwnp_init) {
        log_warn("key_window_node pool was not created, ignore");
        return;
    }

    log_info("destroying key_window_node pool: free %"PRIu32, kwnp.nfree);

    FREEPOOL_DESTROY(kwn, tkwn, &kwnp, next, key_window_node_destroy);
    kwnp_init = false;
}

static void
key_window_node_pool_create(uint32_t max)
{
    struct key_window_node *kwn;

    if (kwnp_init) {
        log_warn("key_window_node pool has already been created, re-creating");
        key_window_node_pool_destroy();
    }

    log_info("creating key_window_node pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&kwnp, max);
    kwnp_init = true;

    FREEPOOL_PREALLOC(kwn, &kwnp, max, next, key_window_node_create);
    if (kwnp.nfree < max) {
        log_crit("cannot preallocate key_window_node pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

static struct key_window_node *
key_window_node_borrow(void)
{
    struct key_window_node *kwn;

    FREEPOOL_BORROW(kwn, &kwnp, next, key_window_node_create);
    if (kwn == NULL) {
        log_debug("borrow key_window_node failed: OOM");
        return NULL;
    }
    key_window_node_reset(kwn);

    return kwn;
}

static void
key_window_node_return(struct key_window_node **key_window_node)
{
    struct key_window_node *kwn = *key_window_node;

    if (kwn == NULL) {
        return;
    }

    FREEPOOL_RETURN(kwn, &kwnp, next);

    *key_window_node = NULL;
}

void
key_window_setup(uint32_t poolsize)
{
    log_info("set up the %s module", KEY_WINDOW_MODULE_NAME);

    if (key_window_init) {
        log_warn("%s has already been setup, overwrite", KEY_WINDOW_MODULE_NAME);
    }

    key_window_node_pool_create(poolsize);
    key_window_size = 0;
    STAILQ_INIT(&q);
    key_window_init = true;
}

void
key_window_teardown(void)
{
    struct key_window_node *kwn, *tkwn;

    log_info("tear down the %s module", KEY_WINDOW_MODULE_NAME);

    if (!key_window_init) {
        log_warn("%s was not setup", KEY_WINDOW_MODULE_NAME);
        return;
    }

    /* free all entries in key_window */
    STAILQ_FOREACH_SAFE(kwn, &q, next, tkwn) {
        key_window_node_return(&kwn);
    }

    key_window_node_pool_destroy();
    key_window_init = false;
}

void
key_window_push(const struct bstring *key)
{
    struct key_window_node *kwn = key_window_node_borrow();

    ASSERT(key->len <= MAX_KEY_LEN);

    cc_memcpy(kwn->key, key->data, key->len);
    kwn->nkey = key->len;
    STAILQ_INSERT_TAIL(&q, kwn, next);
    ++key_window_size;
}

uint32_t
key_window_pop(char *buf)
{
    struct key_window_node *kwn = STAILQ_FIRST(&q);
    uint32_t nkey;

    ASSERT(key_window_size > 0);

    cc_memcpy(buf, kwn->key, kwn->nkey);
    nkey = kwn->nkey;

    STAILQ_REMOVE_HEAD(&q, next);
    key_window_node_return(&kwn);
    --key_window_size;

    return nkey;
}

uint32_t
key_window_len(void)
{
    return key_window_size;
}

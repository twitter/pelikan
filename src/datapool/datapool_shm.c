/*
 * Anonymous shared memory backed datapool.
 * Loses all its contents after closing.
 */
#include "datapool.h"

#include <cc_debug.h>
#include <cc_mm.h>

#include <sys/mman.h>


struct datapool *
datapool_open(const char *path, const char *user_signature, size_t size, int *fresh, bool prefault)
{
    if (path != NULL) {
        log_warn("attempted to open a file-based data pool without"
            "pmem features enabled");
        return NULL;
    }

    if (fresh) {
        *fresh = 1;
    }

    struct datapool *ret = cc_zalloc(size);
#ifdef MADV_HUGEPAGE
    /* USE_HUGEPAGE */
    madvise(ret, size, MADV_HUGEPAGE);
#endif
    return ret;
}

void
datapool_close(struct datapool *pool)
{
    cc_free(pool);
}

void *
datapool_addr(struct datapool *pool)
{
    return pool;
}

size_t
datapool_size(struct datapool *pool)
{
    return cc_alloc_usable_size(pool);
}

/*
 * NOTE: Abstraction in datapool required defining functions below
 *       datapool_get_user_data is currently used only in in pmem implementation
 *       datapool_set_user_data is called during teardown e.g. slab
 */
void
datapool_set_user_data(const struct datapool *pool, const void *user_data, size_t user_size)
{

}

void
datapool_get_user_data(const struct datapool *pool, void *user_data, size_t user_size)
{
    NOT_REACHED();
}

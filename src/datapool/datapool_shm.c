/*
 * Anonymous shared memory backed datapool.
 * Loses all its contents after closing.
 */
#include "datapool.h"

#include <cc_debug.h>
#include <cc_mm.h>

struct datapool *
datapool_open(const char *path, size_t size, int *fresh)
{
    if (path != NULL) {
        log_warn("attempted to open a file-based data pool without"
            "pmem features enabled");
        return NULL;
    }

    if (fresh) {
        *fresh = 1;
    }

    return cc_zalloc(size);
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


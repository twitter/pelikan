/*
 * File-backed datapool.
 * Retains its contents if the pool has been closed correctly.
 *
 */
#include "datapool.h"

#include <cc_mm.h>
#include <cc_debug.h>

#include <inttypes.h>
#include <libpmem.h>
#include <errno.h>

#define DATAPOOL_SIGNATURE ("PELIKAN") /* 8 bytes */
#define DATAPOOL_SIGNATURE_LEN (sizeof(DATAPOOL_SIGNATURE))

/*
 * Size of the data pool header.
 * Big enough to fit all necessary metadata, but most of this size is left
 * unused for future expansion.
 */
#define DATAPOOL_HEADER_LEN 4096
#define DATAPOOL_VERSION 1

#define DATAPOOL_FLAG_DIRTY (1 << 0)
#define DATAPOOL_VALID_FLAGS (DATAPOOL_FLAG_DIRTY)

/*
 * Header at the beginning of the file, it's verified every time the pool is
 * opened.
 */
struct datapool_header {
    uint8_t signature[DATAPOOL_SIGNATURE_LEN];
    uint64_t version;
    uint64_t size;
    uint64_t flags;
    uint8_t unused[DATAPOOL_HEADER_LEN - 32];
};

struct datapool {
    void *addr;

    struct datapool_header *hdr;
    void *user_addr;
    size_t mapped_len;
    int is_pmem;
    int file_backed;
};

static void
datapool_sync_hdr(struct datapool *pool)
{
    int ret = pmem_msync(pool->hdr, DATAPOOL_HEADER_LEN);
    ASSERT(ret == 0);
}

static void
datapool_sync(struct datapool *pool)
{
    int ret = pmem_msync(pool->addr, pool->mapped_len);
    ASSERT(ret == 0);
}

static bool
datapool_valid(struct datapool *pool)
{
    if (memcmp(pool->hdr->signature,
          DATAPOOL_SIGNATURE, DATAPOOL_SIGNATURE_LEN) != 0) {
        log_info("no signature found in datapool");
        return false;
    }

    if (pool->hdr->version != DATAPOOL_VERSION) {
        log_info("incompatible datapool version (is: %d, expecting: %d)",
            pool->hdr->version, DATAPOOL_SIGNATURE);
        return false;
    }

    if (pool->hdr->size == 0) {
        log_error("datapool has 0 size");
        return false;
    }

    if (pool->hdr->size > pool->mapped_len) {
        log_error("datapool has invalid size (is: %d, expecting: %d)",
            pool->mapped_len, pool->hdr->size);
        return false;
    }

    if (pool->hdr->flags & ~DATAPOOL_VALID_FLAGS) {
        log_error("datapool has invalid flags set");
        return false;
    }

    if (pool->hdr->flags & DATAPOOL_FLAG_DIRTY) {
        log_info("datapool has a valid header but is dirty");
        return false;
    }

    return true;
}

static void
datapool_initialize(struct datapool *pool)
{
    log_info("initializing fresh datapool");

    /* 1. clear the header from any leftovers */
    memset(pool->hdr, 0, DATAPOOL_HEADER_LEN);
    datapool_sync_hdr(pool);

    /* 2. fill in the data */
    pool->hdr->version = DATAPOOL_VERSION;
    pool->hdr->size = pool->mapped_len;
    pool->hdr->flags = 0;
    datapool_sync_hdr(pool);

    /* 3. set the signature */
    memcpy(pool->hdr->signature, DATAPOOL_SIGNATURE, DATAPOOL_SIGNATURE_LEN);
    datapool_sync_hdr(pool);
}

static void
datapool_flag_set(struct datapool *pool, int flag)
{
    pool->hdr->flags |= flag;
    datapool_sync_hdr(pool);
}

static void
datapool_flag_clear(struct datapool *pool, int flag)
{
    pool->hdr->flags &= ~flag;
    datapool_sync_hdr(pool);
}

/*
 * Opens, and if necessary initializes, a datapool that resides in the given
 * file. If no file is provided, the pool is allocated through cc_zalloc.
 *
 * The the datapool to retain its contents, the datapool_close() call must
 * finish successfully.
 */
struct datapool *
datapool_open(const char *path, size_t size, int *fresh)
{
    struct datapool *pool = cc_alloc(sizeof(*pool));
    if (pool == NULL) {
        log_error("unable to create allocate memory for pmem mapping");
        goto err_alloc;
    }

    size_t map_size = size + sizeof(struct datapool_header);

    if (path == NULL) { /* fallback to DRAM if pmem is not configured */
        pool->addr = cc_zalloc(map_size);
        pool->mapped_len = map_size;
        pool->is_pmem = 0;
        pool->file_backed = 0;
    } else {
        pool->addr = pmem_map_file(path, map_size, PMEM_FILE_CREATE, 0600,
            &pool->mapped_len, &pool->is_pmem);
        pool->file_backed = 1;
    }

    if (pool->addr == NULL) {
        log_error(path == NULL ? strerror(errno) : pmem_errormsg());
        goto err_map;
    }

    log_info("mapped datapool %s with size %llu, is_pmem: %d",
        path, pool->mapped_len, pool->is_pmem);

    pool->hdr = pool->addr;
    pool->user_addr = (uint8_t *)pool->addr + sizeof(struct datapool_header);

    if (fresh) {
        *fresh = 0;
    }

    if (!datapool_valid(pool)) {
        if (fresh) {
            *fresh = 1;
        }

        datapool_initialize(pool);
    }

    datapool_flag_set(pool, DATAPOOL_FLAG_DIRTY);

    return pool;

err_map:
    cc_free(pool);
err_alloc:
    return NULL;
}

void
datapool_close(struct datapool *pool)
{
    datapool_sync(pool);
    datapool_flag_clear(pool, DATAPOOL_FLAG_DIRTY);

    if (pool->file_backed) {
        int ret = pmem_unmap(pool->addr, pool->mapped_len);
        ASSERT(ret == 0);
    } else {
        cc_free(pool->addr);
    }

    cc_free(pool);
}

void *
datapool_addr(struct datapool *pool)
{
    return pool->user_addr;
}

size_t
datapool_size(struct datapool *pool)
{
    return pool->mapped_len - sizeof(struct datapool_header);
}


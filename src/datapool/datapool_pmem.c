/*
 * File-backed datapool.
 * Retains its contents if the pool has been closed correctly.
 *
 */
#include "datapool.h"

#include <cc_mm.h>
#include <cc_debug.h>

#include <libpmem.h>
#include <errno.h>


static void
datapool_sync_hdr(struct datapool *pool)
{
    int ret = pmem_msync(pool->hdr, DATAPOOL_HEADER_LEN);
    if (ret != 0) {
        printf("%s\n", pmem_errormsg());
        log_error(pmem_errormsg());
    }
    ASSERT(ret == 0);
}

static void
datapool_sync(struct datapool *pool)
{
    int ret = pmem_msync(pool->addr, pool->mapped_len);
    ASSERT(ret == 0);
}

static bool
datapool_valid_user_signature(struct datapool *pool, const char *user_name)
{
    if (cc_strcmp(pool->hdr->user_signature, user_name)) {
        return false;
    }
    return true;
}

static bool
datapool_valid(struct datapool *pool)
{
    return false;


    if (cc_memcmp(pool->hdr->signature,
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
datapool_initialize(struct datapool *pool, const char *user_name)
{
    log_info("initializing fresh datapool");

    /* 1. clear the header from any leftovers */
    cc_memset(pool->hdr, 0, DATAPOOL_HEADER_LEN);

    /* 2. fill in the data */
    pool->hdr->version = DATAPOOL_VERSION;
    pool->hdr->size = pool->mapped_len;
    pool->hdr->flags = 0;
    cc_memcpy(pool->hdr->user_signature, user_name, cc_strlen(user_name));

    /* 3. set the signature */
    cc_memcpy(pool->hdr->signature, DATAPOOL_SIGNATURE, DATAPOOL_SIGNATURE_LEN);
    datapool_sync_hdr(pool);
}

static void
datapool_flag_set(struct datapool *pool, uint64_t flag)
{
    pool->hdr->flags |= flag;
    datapool_sync_hdr(pool);
}

static void
datapool_flag_clear(struct datapool *pool, uint64_t flag)
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
datapool_open(const char *path, const char *user_signature, size_t size, int *fresh, bool prefault)
{
    struct datapool *pool = cc_alloc(sizeof(*pool));
    if (pool == NULL) {
        log_error("unable to create allocate memory for pmem mapping");
        goto err_alloc;
    }

    if (user_signature == NULL) {
        log_error("empty user signature");
        goto err_map;
    }

    if (cc_strnlen(user_signature, DATAPOOL_USER_LAYOUT_LEN) == DATAPOOL_USER_LAYOUT_LEN ) {
        log_error("user signature is too long %zu", cc_strlen(user_signature));
        goto err_map;
    }

    size_t map_size = size + sizeof(struct datapool_header);

    if (path == NULL) { /* fallback to DRAM if pmem is not configured */
        log_error("compiled with PMem, but no path provided, fall back to DRAM");
        pool->addr = cc_zalloc(map_size);
        pool->mapped_len = map_size;
        pool->is_pmem = 0;
        pool->file_backed = 0;
    } else {
        pool->addr = pmem_map_file(path, map_size, PMEM_FILE_CREATE, 0600,
                &pool->mapped_len, &pool->is_pmem);
        if (pool->addr == NULL) {
            log_warn("%s, now create file with size %zu", pmem_errormsg(), map_size);
            pool->addr = pmem_map_file(path, 0, PMEM_FILE_CREATE, 0600,
                    &pool->mapped_len, &pool->is_pmem);
        }
        pool->file_backed = 1;
    }

    if (pool->addr == NULL) {
        log_error(path == NULL ? strerror(errno) : pmem_errormsg());
        goto err_map;
    }

    if (prefault) {
        log_info("prefault datapool");
        volatile char *cur_addr = pool->addr;
        char *addr_end = (char *)cur_addr + map_size;
        for (; cur_addr < addr_end; cur_addr += PAGE_SIZE) {
            *cur_addr = *cur_addr;
        }
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

        datapool_initialize(pool, user_signature);
    } else if (!datapool_valid_user_signature(pool, user_signature)) {
        log_error("wrong user signature (%s) used for pool", user_signature);
        goto err_map_adr;
    }

    datapool_flag_set(pool, DATAPOOL_FLAG_DIRTY);

    return pool;

err_map_adr:
    if (pool->file_backed) {
        int ret = pmem_unmap(pool->addr, pool->mapped_len);
        ASSERT(ret == 0);
    } else {
        cc_free(pool->addr);
    }
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

void
datapool_set_user_data(const struct datapool *pool, const void *user_data, size_t user_size)
{
    ASSERT(user_size < DATAPOOL_USER_HEADER_LEN - DATAPOOL_USER_LAYOUT_LEN);
    cc_memcpy(pool->hdr->user_data, user_data, user_size);
}

void
datapool_get_user_data(const struct datapool *pool, void *user_data, size_t user_size)
{
    ASSERT(user_size < DATAPOOL_USER_HEADER_LEN - DATAPOOL_USER_LAYOUT_LEN);
    cc_memcpy(user_data, pool->hdr->user_data, user_size);
}

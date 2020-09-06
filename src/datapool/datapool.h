#pragma once

#include <inttypes.h>
#include <stdbool.h>
#include <stddef.h>

/*
 * Size of the data pool header.
 * Big enough to fit all necessary metadata, but most of this size is left
 * unused for future expansion.
 */

#define DATAPOOL_INTERNAL_HEADER_LEN 2048
#define DATAPOOL_USER_LAYOUT_LEN 48
#define DATAPOOL_USER_HEADER_LEN 2048
#define DATAPOOL_HEADER_LEN                                                    \
    (DATAPOOL_INTERNAL_HEADER_LEN + DATAPOOL_USER_HEADER_LEN)
#define DATAPOOL_VERSION 1

#define DATAPOOL_FLAG_DIRTY (1 << 0)
#define DATAPOOL_VALID_FLAGS (DATAPOOL_FLAG_DIRTY)

#define PAGE_SIZE 4096

#define DATAPOOL_SIGNATURE ("PELIKAN") /* 8 bytes */
#define DATAPOOL_SIGNATURE_LEN (sizeof(DATAPOOL_SIGNATURE))


/*
 * Header at the beginning of the file, it's verified every time the pool is
 * opened.
 */
struct datapool_header {
    /* TODO(jason): need to persist datapool creation time for TTL */
    uint8_t signature[DATAPOOL_SIGNATURE_LEN];
    uint64_t version;
    uint64_t size;
    uint64_t flags;
    uint8_t unused[DATAPOOL_INTERNAL_HEADER_LEN - 32];

    uint8_t user_signature[DATAPOOL_USER_LAYOUT_LEN];
    uint8_t user_data[DATAPOOL_USER_HEADER_LEN - DATAPOOL_USER_LAYOUT_LEN];
};

/* we need time to be persisted so that after restart we know the difference
 * between current proc_time and prev proc_time */
struct datapool {
    void *addr; /* TODO(jason): maybe call it _mmap_address? Given this is
                   internal var */

    struct datapool_header *hdr; /* TODO(jason): make this non-pointer? */
    void *user_addr; /* TODO(jason): maybe call it data_addr? */
    size_t mapped_len;
    int is_pmem;
    int file_backed;
};

/* TODO(jason) turn it into a real shared-memory implementation */

struct datapool *datapool_open(const char *path, const char *user_signature,
        size_t size, int *fresh, bool prefault);
void datapool_close(struct datapool *pool);

void *datapool_addr(struct datapool *pool);
size_t datapool_size(struct datapool *pool);
void datapool_set_user_data(
        const struct datapool *pool, const void *user_data, size_t user_size);
void datapool_get_user_data(
        const struct datapool *pool, void *user_data, size_t user_size);

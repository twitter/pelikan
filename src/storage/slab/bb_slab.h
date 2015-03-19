#ifndef _BB_SLAB_H_
#define _BB_SLAB_H_

#include <storage/slab/bb_item.h>
#include <time/bb_time.h>

#include <cc_define.h>
#include <cc_queue.h>
#include <cc_util.h>

#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define SLAB_MAGIC      0xdeadbeef
#define SLAB_HDR_SIZE   offsetof(struct slab, data)
#define SLAB_MIN_SIZE   ((size_t) 512)
#define SLAB_MAX_SIZE   ((size_t) (128 * MiB))
#define SLAB_SIZE       MiB

/* Eviction options */
#define EVICT_NONE    0x00 /* throw OOM, no eviction */
#define EVICT_RS      0x01 /* random slab eviction */
#define EVICT_CS      0x02 /* lrc (least recently created) slab eviction */
#define EVICT_INVALID 0x04 /* go no further! */

/* The defaults here are placeholder values for now */
/*          name             type                default          description */
#define SLAB_OPTION(ACTION)                                                                     \
    ACTION( prealloc,        OPTION_TYPE_BOOL,   "yes",           "Allocate slabs ahead of time")\
    ACTION( evict_opt,       OPTION_TYPE_UINT,   str(EVICT_NONE), "Eviction strategy"           )\
    ACTION( use_freeq,       OPTION_TYPE_BOOL,   "yes",           "Use items in free queue?"    )\
    ACTION( slab_size,       OPTION_TYPE_UINT,   str(MiB),        "Slab size"                   )\
    ACTION( chunk_size,      OPTION_TYPE_UINT,   str(KiB),        "Chunk size"                  )\
    ACTION( maxbytes,        OPTION_TYPE_UINT,   str(GiB),        "Maximum bytes allocated"     )\
    ACTION( profile,         OPTION_TYPE_STR,    NULL,            "Slab profile"                )\
    ACTION( profile_last_id, OPTION_TYPE_UINT,   "0",             "Last id in slab profile"     )\
    ACTION( use_cas,         OPTION_TYPE_BOOL,   "yes",           "CAS enabled for slabbed mm"  )

/*
 * Every slab (struct slab) in the cache starts with a slab header
 * followed by slab data. The slab data is essentially a collection of
 * contiguous, equal sized items (struct item)
 *
 * An item is owned by a slab and a slab is owned by a slabclass
 *
 *   <------------------------ slab_size ------------------------->
 *   +---------------+--------------------------------------------+
 *   |  slab header  |              slab data                     |
 *   | (struct slab) |      (contiguous equal sized items)        |
 *   +---------------+--------------------------------------------+
 *   ^               ^
 *   |               |
 *   \               |
 *   slab            \
 *                   slab->data
 *
 * Note: keep struct slab 8-byte aligned so that item chunks always start on
 *       8-byte aligned boundary.
 */
struct slab {
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    uint32_t          magic;    /* slab magic (const) */
#endif
    uint8_t           id;       /* slabclass id */
    uint8_t           unused;   /* unused */
    uint16_t          refcount; /* # concurrent users */
    TAILQ_ENTRY(slab) s_tqe;    /* link in slab lruq */
    rel_time_t        utime;    /* last update time in secs */
    uint32_t          padding;  /* unused */
    uint8_t           data[1];  /* opaque data */
};

TAILQ_HEAD(slab_tqh, slab);

/* Queues for handling items */
SLIST_HEAD(item_slh, item);

/*
 * Every class (struct slabclass) is a collection of slabs that can serve
 * items of a given maximum size. Every slab in the cache is identified by a
 * unique unsigned 8-bit id, which also identifies its owner slabclass
 *
 * Slabs that belong to a given class are reachable through slabq. Slabs
 * across all classes are reachable through the slabtable and slab lruq.
 *
 * We use unslabbed_free_item as a marker for the next available, unallocated
 * item in the current slab. Items that are available for reuse (i.e. allocated
 * and then freed) are kept track by free_itemq
 *
 * slabclass[]:
 *
 *  +-------------+
 *  |             |
 *  |             |
 *  |             |
 *  |   class 0   |
 *  |             |
 *  |             |
 *  |             |
 *  +-------------+
 *  |             |  ----------------------------------------------------------+
 *  |             | /                                              (last slab) |
 *  |             |/    +---------------+-------------------+    +-------------v-+-------------------+
 *  |             |     |               |                   |    |               |                   |
 *  |   class 1   |     |  slab header  |     slab data     |    |  slab header  |     slab data     |
 *  |             |     |               |                   |    |               |                   |--+
 *  |             |\    +---------------+-------------------+    +---------------+-------------------+  |
 *  |             | \                                                                                   //
 *  |             |  ----> (freeq)
 *  +-------------+
 *  |             |  -----------------+
 *  |             | /     (last slab) |
 *  |             |/    +-------------v-+-------------------+
 *  |             |     |               |                   |
 *  |   class 2   |     |  slab header  |     slab data     |
 *  |             |     |               |                   |--+
 *  |             |\    +---------------+-------------------+  |
 *  |             | \                                          //
 *  |             |  ----> (freeq)
 *  +-------------+
 *  |             |
 *  |             |
 *  .             .
 *  .    ....     .
 *  .             .
 *  |             |
 *  |             |
 *  +-------------+
 *            |
 *            |
 *            //
 */
struct slabclass {
    uint32_t        nitem;                 /* # item per slab (const) */
    size_t          size;                  /* item size (const) */

    uint32_t        nfree_itemq;           /* # free item q */
    struct item_slh free_itemq;            /* free item q */

    uint32_t        nfree_item;            /* # free item (in current slab) */
    struct item     *next_item_in_slab;    /* next free item (in current slab, not freeq) */
};

/*
 * Slabclass id is an unsigned byte. So, maximum number of slab classes
 * cannot exceeded 256
 *
 * We use id = 255 as an invalid id and id = 0 for aggregation. This means
 * that we can have at most 254 usable slab classes
 */
#define SLABCLASS_MIN_ID        1
#define SLABCLASS_MAX_ID        (UCHAR_MAX - 1)
#define SLABCLASS_INVALID_ID    UCHAR_MAX
#define SLABCLASS_MAX_IDS       UCHAR_MAX

extern size_t slab_size_setting;
extern bool use_cas;

size_t slab_size(void);
void slab_print(void);
void slab_acquire_refcount(struct slab *slab);
void slab_release_refcount(struct slab *slab);
size_t slab_item_size(uint8_t id);
uint8_t slab_id(size_t size);

rstatus_t slab_setup(size_t setup_slab_size, bool setup_use_cas, bool setup_prealloc,
                     int setup_evict_opt, bool setup_use_freeq, size_t setup_chunk_size,
                     size_t setup_maxbytes, char *setup_profile, uint8_t setup_profile_last_id);
void slab_teardown(void);

struct item *slab_get_item(uint8_t id);
void slab_put_item(struct item *it, uint8_t id);

#endif

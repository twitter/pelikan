#pragma once

#include <cc_queue.h>

#include <limits.h>
#include <stdint.h>

/* Queues for handling items */
struct item;
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

extern struct slabclass slabclass[SLABCLASS_MAX_ID + 1];  /* collection of slabs bucketed by slabclass */

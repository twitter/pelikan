#ifndef _BB_CUCKOO_H_
#define _BB_CUCKOO_H_

#include <cuckoo/bb_item.h>
#include <time/bb_time.h>

#include <cc_define.h>
#include <cc_bstring.h>

#include <inttypes.h>
#include <stdbool.h>

#define CUCKOO_DISPLACE 2
#define CUCKOO_ITEM_SIZE 64
#define CUCKOO_NITEM 1024

/*          name                type                default                 description */
#define CUCKOO_OPTION(ACTION)                                                                        \
    ACTION( cuckoo_displace,    OPTION_TYPE_UINT,   str(CUCKOO_DISPLACE),   "# displaces allowed"   )\
    ACTION( cuckoo_item_size,   OPTION_TYPE_UINT,   str(CUCKOO_ITEM_SIZE),  "item size (inclusive)" )\
    ACTION( cuckoo_nitem,       OPTION_TYPE_UINT,   str(CUCKOO_NITEM),      "# items allocated"     )

/*          name                    type            description */
#define CUCKOO_METRIC(ACTION)                                               \
    ACTION( cuckoo_lookup,          METRIC_COUNTER, "# lookups"            )\
    ACTION( cuckoo_lookup_found,    METRIC_COUNTER, "# positive lookups"   )\
    ACTION( cuckoo_lookup_notfound, METRIC_COUNTER, "# negative lookups"   )\
    ACTION( cuckoo_insert,          METRIC_COUNTER, "# inserts"            )

rstatus_t cuckoo_setup(size_t size, uint32_t item);
void cuckoo_teardown(void);

struct item * cuckoo_lookup(struct bstring *key);
void cuckoo_insert(struct bstring *key, struct val *val, rel_time_t expire);
bool cuckoo_delete(struct bstring *key);
void cuckoo_update(struct item *it, struct val *val, rel_time_t expire);

#endif

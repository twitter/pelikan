#ifndef _BB_CUCKOO_H_
#define _BB_CUCKOO_H_

#include <cuckoo/bb_item.h>
#include <time/bb_time.h>

#include <cc_define.h>
#include <cc_bstring.h>

#include <inttypes.h>
#include <stdbool.h>

#define CUCKOO_POLICY_RANDOM 1
#define CUCKOO_POLICY_EXPIRE 2


#define CUCKOO_DISPLACE 2
#define CUCKOO_ITEM_SIZE 64
#define CUCKOO_NITEM 1024
#define CUCKOO_POLICY CUCKOO_POLICY_RANDOM

/*          name                type                default                 description */
#define CUCKOO_OPTION(ACTION)                                                                        \
    ACTION( cuckoo_displace,    OPTION_TYPE_UINT,   str(CUCKOO_DISPLACE),   "# displaces allowed"   )\
    ACTION( cuckoo_item_cas,    OPTION_TYPE_BOOL,   "yes",                  "support cas in items"  )\
    ACTION( cuckoo_item_size,   OPTION_TYPE_UINT,   str(CUCKOO_ITEM_SIZE),  "item size (inclusive)" )\
    ACTION( cuckoo_nitem,       OPTION_TYPE_UINT,   str(CUCKOO_NITEM),      "# items allocated"     )\
    ACTION( cuckoo_policy,      OPTION_TYPE_UINT,   str(CUCKOO_POLICY),     "evict policy"          )

rstatus_t cuckoo_setup(size_t size, uint32_t item, uint32_t policy);
void cuckoo_teardown(void);

struct item * cuckoo_lookup(struct bstring *key);
rstatus_t cuckoo_insert(struct bstring *key, struct val *val, rel_time_t expire);
rstatus_t cuckoo_update(struct item *it, struct val *val, rel_time_t expire);
bool cuckoo_delete(struct bstring *key);

#endif

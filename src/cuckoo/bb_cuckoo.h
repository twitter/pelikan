#ifndef _BB_CUCKOO_H_
#define _BB_CUCKOO_H_

#include <inttypes.h>

#include <cc_string.h>
#include <bb_time.h>

#include <bb_item.h>

rstatus_t cuckoo_setup(size_t size, uint32_t item);
void cuckoo_teardown(void);

struct item * cuckoo_lookup(struct bstring *key);
void cuckoo_insert(struct bstring *key, struct val *val, rel_time_t expire);

#endif

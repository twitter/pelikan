#pragma once

#include <stdint.h>

/*
 * The kc_map module provides a utility for counting the frequency at which keys
 * appear. When a key is sampled, it should be incremented in the table after
 * enqueueing it. When a key is dequeued, it should be decremented from the
 * table.
 */

struct bstring;

/* setup/teardown module */
void kc_map_setup(uint32_t size, uint32_t poolsize);
void kc_map_teardown(void);

/* increment count and return count, insert into table if count == 0 */
uint32_t kc_map_incr(const struct bstring *key);

/* decrement count, remove from table if count == 0 after decr */
void kc_map_decr(const struct bstring *key);

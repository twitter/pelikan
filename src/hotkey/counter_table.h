#pragma once

#include <stdint.h>

/*
 * The counter_table module provides a utility for counting the frequency at
 * which keys appear. When a key is sampled, it should be incremented in the
 * table after enqueueing it. When a key is dequeued, it should be decremented
 * from the table.
 */

/* setup/teardown module */
void counter_table_setup(uint32_t size, uint32_t poolsize);
void counter_table_teardown(void);

/* increment count and return count, insert into table if count == 0 */
uint32_t counter_table_incr(char *key, uint32_t nkey);

/* decrement count, remove from table if count == 0 after decr */
void counter_table_decr(char *key, uint32_t nkey);

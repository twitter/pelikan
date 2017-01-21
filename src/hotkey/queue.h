#pragma once

#include <stdint.h>

/*
 * The queue module provides a FIFO queue interface to help with bookkeeping the
 * last N keys provided for hotkey detection.
 */

void queue_push(char *key, uint32_t nkey);
uint32_t queue_pop(char *buf); /* returns length of key */
uint32_t queue_len(void);

/* Setup/teardown functions, must be called before module is used */
void queue_setup(uint32_t poolsize);
void queue_teardown(void);

#pragma once

#include <stdint.h>

/*
 * The key_window module provides a FIFO key_window interface to help with bookkeeping the
 * last N keys provided for hotkey detection.
 */

struct bstring;

void key_window_push(const struct bstring *key);
uint32_t key_window_pop(char *buf); /* returns length of key */
uint32_t key_window_len(void);

/* Setup/teardown functions, must be called before module is used */
void key_window_setup(uint32_t poolsize);
void key_window_teardown(void);

#pragma once

#include <cc_option.h>

#include <stdbool.h>
#include <stdint.h>

/* TODO(kevyang): add stats for hotkey module */

#define HOTKEY_WINDOW_SIZE     10000 /* keep last 10000 keys sampled by default */
#define HOTKEY_RATE            100   /* sample one in every 100 keys by default */
#define HOTKEY_THRESHOLD_RATIO 0.01  /* signal hotkey if key takes up >= 0.01 of all keys by default */
#define HOTKEY_THRESHOLD       (uint32_t)(HOTKEY_THRESHOLD_RATIO * HOTKEY_WINDOW_SIZE)

/*          name                    type                default                 description */
#define HOTKEY_OPTION(ACTION)                                                                                 \
    ACTION( hotkey_enable,          OPTION_TYPE_BOOL,   false,                  "use hotkey detection?"      )\
    ACTION( hotkey_sample_size,     OPTION_TYPE_UINT,   HOTKEY_WINDOW_SIZE,     "number of keys to maintain" )\
    ACTION( hotkey_sample_rate,     OPTION_TYPE_UINT,   HOTKEY_RATE,            "hotkey sample ratio"        )\
    ACTION( hotkey_threshold_ratio, OPTION_TYPE_UINT,   HOTKEY_THRESHOLD_RATIO, "threshold for hotkey signal")

typedef struct {
    HOTKEY_OPTION(OPTION_DECLARE)
} hotkey_options_st;

extern bool hotkey_enabled;

void hotkey_setup(hotkey_options_st *options);
void hotkey_teardown(void);
bool hotkey_sample(const struct bstring *key);

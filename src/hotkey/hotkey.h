#pragma once

#include <cc_option.h>

#include <stdbool.h>
#include <stdint.h>

/* TODO(kevyang): add stats for hotkey module */

#define HOTKEY_NSAMPLE   10000 /* keep last 10000 keys sampled by default */
#define HOTKEY_RATE      100   /* sample one in every 100 keys by default */
#define HOTKEY_THRESHOLD 10    /* signal for hotkey if 10 or more keys in sample by default */

/*          name                type                default             description */
#define HOTKEY_OPTION(ACTION)                                                                    \
    ACTION( hotkey_enable,      OPTION_TYPE_BOOL,   false,              "use hotkey detection?"      )\
    ACTION( hotkey_sample_size, OPTION_TYPE_UINT,   HOTKEY_NSAMPLE,     "number of keys to maintain" )\
    ACTION( hotkey_sample_rate, OPTION_TYPE_UINT,   HOTKEY_RATE,        "hotkey sample ratio"        )\
    ACTION( hotkey_threshold,   OPTION_TYPE_UINT,   HOTKEY_THRESHOLD,   "threshold for hotkey signal")

typedef struct {
    HOTKEY_OPTION(OPTION_DECLARE)
} hotkey_options_st;

extern bool hotkey_enabled;

void hotkey_setup(hotkey_options_st *options);
void hotkey_teardown(void);

#define hotkey_sample(key, nkey) do { \
    if (hotkey_enabled) {                            \
        _hotkey_sample(key, nkey);                   \
    }                                                \
} while (0)

bool _hotkey_sample(char *key, uint32_t nkey);

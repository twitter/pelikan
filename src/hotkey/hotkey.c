#include "hotkey.h"

#include "constant.h"
#include "kc_map.h"
#include "key_window.h"

#include <cc_bstring.h>
#include <cc_debug.h>

#define HOTKEY_MODULE_NAME "hotkey::hotkey"

bool hotkey_enabled = false;

static uint64_t hotkey_counter;

static bool hotkey_init = false;
static uint32_t hotkey_window_size = HOTKEY_WINDOW_SIZE;
static uint32_t hotkey_rate = HOTKEY_RATE;
static uint32_t hotkey_threshold = HOTKEY_THRESHOLD;
static uint32_t hotkey_window_size_cur = 0;

void
hotkey_setup(hotkey_options_st *options)
{
    log_info("Set up the %s module", HOTKEY_MODULE_NAME);

    if (options != NULL) {
        hotkey_enabled = option_bool(&options->hotkey_enable);
        hotkey_window_size = option_uint(&options->hotkey_sample_size);
        hotkey_rate = option_uint(&options->hotkey_sample_rate);
        hotkey_threshold = (uint32_t)(option_fpn(&options->hotkey_threshold_ratio) * hotkey_window_size);
    }

    hotkey_window_size_cur = 0;
    hotkey_counter = 0;
    key_window_setup(hotkey_window_size);
    /* TODO: determine whether table size should be a tuneable parameter */
    kc_map_setup(hotkey_window_size, hotkey_window_size);
    hotkey_init = true;
}

void
hotkey_teardown(void)
{
    log_info("Tear down the %s module", HOTKEY_MODULE_NAME);

    if (!hotkey_init) {
        log_warn("%s was not setup", HOTKEY_MODULE_NAME);
        return;
    }

    hotkey_enabled = false;
    key_window_teardown();
    kc_map_teardown();
    hotkey_init = false;
}

bool
hotkey_sample(const struct bstring *key)
{
    if (++hotkey_counter % hotkey_rate == 0) {
        /* sample this key */
        uint32_t freq;

        if (key_window_len() == hotkey_window_size) {
            char buf[MAX_KEY_LEN];
            struct bstring popped;

            popped.data = buf;

            /* pop from key_window, decrement in counter table */
            popped.len = key_window_pop(popped.data);
            kc_map_decr(&popped);
        }

        key_window_push(key);
        freq = kc_map_incr(key);

        return freq >= hotkey_threshold;
    }

    return false;
}

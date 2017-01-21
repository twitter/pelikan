#include "hotkey.h"

#include "constant.h"
#include "counter_table.h"
#include "queue.h"

#include <cc_debug.h>

#define HOTKEY_MODULE_NAME "hotkey::hotkey"

bool hotkey_enabled = false;

static uint64_t hotkey_counter;

static bool hotkey_init = false;
static uint32_t hotkey_nsample = HOTKEY_NSAMPLE;
static uint32_t hotkey_rate = HOTKEY_RATE;
static uint32_t hotkey_threshold = HOTKEY_THRESHOLD;
static uint32_t hotkey_nsample_cur = 0;

void
hotkey_setup(hotkey_options_st *options)
{
    log_info("Set up the %s module", HOTKEY_MODULE_NAME);

    if (options != NULL) {
        hotkey_enabled = option_bool(&options->hotkey_enable);
        hotkey_nsample = option_uint(&options->hotkey_sample_size);
        hotkey_rate = option_uint(&options->hotkey_sample_rate);
        hotkey_threshold = option_uint(&options->hotkey_threshold);
    }

    hotkey_nsample_cur = 0;
    hotkey_counter = 0;
    queue_setup(hotkey_nsample);
    /* TODO: determine whether table size should be a tuneable parameter */
    counter_table_setup(hotkey_nsample, hotkey_nsample);
    hotkey_init = true;
}

void
hotkey_teardown(void)
{
    log_info("Tear down the %s module", HOTKEY_MODULE_NAME);

    if (!hotkey_init) {
        log_warn("%s was not setup", HOTKEY_MODULE_NAME);
    }

    hotkey_enabled = false;
    queue_teardown();
    counter_table_teardown();
    hotkey_init = false;
}

bool
_hotkey_sample(char *key, uint32_t nkey)
{
    if (++hotkey_counter % hotkey_rate == 0) {
        /* sample this key */
        uint32_t freq;

        if (queue_len() == hotkey_nsample) {
            char buf[MAX_KEY_LEN];
            uint32_t len;

            /* pop from queue, decrement in counter table */
            len = queue_pop(buf);
            counter_table_decr(buf, len);
        }

        queue_push(key, nkey);
        freq = counter_table_incr(key, nkey);

        return freq >= hotkey_threshold;
    }

    return false;
}

#ifndef _BB_ITEM_H_
#define _BB_ITEM_H_

#include <time/bb_time.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_util.h>

#include <inttypes.h>
#include <stdbool.h>
#include <stddef.h>

/*          name            type            description */
#define ITEM_METRIC(ACTION)                                         \
    ACTION( item_val_curr,  METRIC_GAUGE,   "#B stored in vals"    )\
    ACTION( item_key_curr,  METRIC_GAUGE,   "#B stored in keys"    )\
    ACTION( item_data_curr, METRIC_GAUGE,   "#B stored"            )\
    ACTION( item_curr,      METRIC_GAUGE,   "# items"              )\
    ACTION( item_displace,  METRIC_COUNTER, "# displace of items"  )\
    ACTION( item_evict,     METRIC_COUNTER, "# evicted items"      )\
    ACTION( item_expire,    METRIC_COUNTER, "# expired items"      )\
    ACTION( item_insert,    METRIC_COUNTER, "# item inserts"       )\
    ACTION( item_delete,    METRIC_COUNTER, "# item deletes"       )\


static bool cas_enabled = true;
static uint64_t cas_val; /* incr'ed before assignment, 0 is a special value */

/*
 * val_type_t and struct val makes it easier to use one object to communicate
 * values between in-memory storage and other modules
 *
 * max value length is 127 given the encoding scheme
 */
typedef enum val_type {
    VAL_TYPE_INT=1,
    VAL_TYPE_STR=2,
    VAL_TYPE_SENTINEL
} val_type_t;

struct val {
    val_type_t type;
    union {
        struct bstring vstr;
        uint64_t vint;
    };
};


/*
 * Every item chunk in the slimcache starts with an header (struct item)
 * followed by item data. All chunks have the same size and are aligned.
 *
 *   <-----------------------item size------------------>
 *   +---------------+----------------------------------+
 *   |               |                                  |
 *   |  item header  |          item payload            |
 *   | (struct item) |         ...      ...             |
 *   +---------------+-------+-------+------------------+
 *   ^               ^       ^       ^
 *   |               |       |       |
 *   |               |       |       |
 *   |               |       |       |
 *   |               |       |       \
 *   |               |       |       ITEM_VAL_POS()
 *   |               |       \
 *   \               |       ITEM_KEY_POS()
 *   item            \
 *                   item->data, (if enabled) ITEM_CAS_POS()
 *
 * item->data is followed by:
 * - 8-byte cas, if ITEM_CAS flag is set
 * - key as a binary string (no terminating '\0')
 * - value as a binary string (no terminating '\0')
 */

struct item {
  rel_time_t expire;
  uint8_t    klen;
  uint8_t    vlen;
  uint8_t    data[1];
};

#define KEY_MAXLEN 255
#define CAS_VAL_MIN 1
#define MIN_ITEM_CHUNK_SIZE CC_ALIGN(sizeof(struct item) + 2, CC_ALIGNMENT)
#define ITEM_HDR_SIZE sizeof(struct item)

#define ITEM_CAS_POS(it) ((it)->data)
#define ITEM_KEY_POS(it) ((it)->data + cas_enabled * sizeof(uint64_t))
#define ITEM_VAL_POS(it) (ITEM_KEY_POS(it) + (it)->klen)

#define ITEM_OVERHEAD offsetof(struct item, data) + cas_enabled * sizeof(uint64_t)

static inline uint8_t
item_klen(struct item *it)
{
    return it->klen;
}

static inline uint32_t
item_flag(struct item *it)
{
    return (uint32_t) 0;
}

static inline uint64_t
item_cas(struct item *it)
{
    if (!cas_enabled) {
        return CAS_VAL_MIN; /* when cas disabled, still allow gets to work */
    }

    return (*(uint64_t *)ITEM_CAS_POS(it));
}

static inline void
item_key(struct bstring *key, struct item *it)
{
    key->len = it->klen;
    key->data = ITEM_KEY_POS(it);
}

static inline bool
item_matched(struct item *it, struct bstring *key)
{
    if (key->len != it->klen) {
        return false;
    }

    return !cc_bcmp(ITEM_KEY_POS(it), key->data, key->len);
}

static inline bool
item_valid(struct item *it)
{
    return (it->expire > time_now());
}

static inline bool
item_expired(struct item *it)
{
    return (it->expire > time_now() && it->expire > 0);
}

static inline bool
item_cas_valid(struct item *it, uint64_t cas)
{
    if (!cas_enabled) {
        return true; /* always succeed when cas is disabled */
    }

    if (item_cas(it) == cas) {
        return true;
    }

    return false;
}

static inline val_type_t
item_vtype(struct item *it)
{
    if (it->vlen == 0) {
        return VAL_TYPE_INT;
    } else {
        return VAL_TYPE_STR;
    }
}

static inline uint8_t
item_vlen(struct item *it)
{
    return (it->vlen == 0) ? sizeof(uint64_t) : it->vlen;
}

static inline uint32_t
item_datalen(struct item *it)
{
    return (uint32_t)item_klen(it) + item_vlen(it);
}

static inline void
item_value_str(struct bstring *str, struct item *it)
{
    str->len = item_vlen(it);
    str->data = ITEM_VAL_POS(it);
}

static inline uint64_t
item_value_int(struct item *it)
{
    return *(uint64_t *)ITEM_VAL_POS(it);
}

static inline void
item_val(struct val *val, struct item *it)
{
    val->type = item_vtype(it);

    if (item_vtype(it) == VAL_TYPE_INT) {
        val->vint = item_value_int(it);
    } else if (item_vtype(it) == VAL_TYPE_STR) {
        item_value_str(&val->vstr, it);
    } else {
        NOT_REACHED();
    }
}

static inline void
item_value_update(struct item *it, struct val *val)
{
    if (cas_enabled) {
        cas_val++;
        *(uint64_t *)ITEM_CAS_POS(it) = cas_val;
    }

    if (val->type == VAL_TYPE_INT) {
        it->vlen = 0;
        *(uint64_t *)ITEM_VAL_POS(it) = val->vint;
    } else if (val->type == VAL_TYPE_STR) {
        it->vlen = (uint8_t)val->vstr.len;
        cc_memcpy(ITEM_VAL_POS(it), val->vstr.data, val->vstr.len);
    }
}

static inline void
item_update(struct item *it, struct val *val, rel_time_t expire)
{
    it->expire = expire;
    item_value_update(it, val);
}

static inline void
item_set(struct item *it, struct bstring *key, struct val *val, rel_time_t expire)
{
    it->klen = (uint8_t)key->len;
    cc_memcpy(ITEM_KEY_POS(it), key->data, key->len);
    item_update(it, val, expire);
}

static inline void
item_delete(struct item *it)
{
    it->expire = 0;
}

static inline void
item_setup(bool cas)
{
    cas_enabled = cas;
}

static inline void
item_teardown(void)
{
    return;
}
#endif /* _BB_ITEM_H_ */

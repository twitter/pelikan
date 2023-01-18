#include "hashmap.h"

#include <cc_debug.h>


#define HM_BODY(_hm) ((char *)(_hm) + HASHMAP_HEADER_SIZE)
#define HM_END(_hm) ((char *)(_hm) + HASHMAP_HEADER_SIZE + HM_NBODY(hm))


static inline uint8_t
_entry_klen(char *entry)
{
    return *((uint8_t *)(entry));
}

static inline uint8_t
_entry_vlen(char *entry)
{
    return *((uint8_t *)((entry) + sizeof(uint8_t)));
}

static inline uint32_t
_entry_nbyte(char *entry)
{
    return (uint32_t)HASHMAP_ENTRY_HEADER_SIZE + _entry_klen(entry) +
        _entry_vlen(entry);
}

static inline char *
_entry_key(char *entry)
{
    return entry + HASHMAP_ENTRY_HEADER_SIZE;
}

static inline char *
_entry_val(char *entry)
{
    return entry + HASHMAP_ENTRY_HEADER_SIZE + _entry_klen(entry);
}

static inline void
_entry_set(char *entry, const struct bstring *key, const struct bstring *val)
{
    *((uint8_t *)entry) = key->len;
    *((uint8_t *)entry + sizeof(uint8_t)) = val->len;
    cc_memcpy(entry + HASHMAP_ENTRY_HEADER_SIZE, key->data, key->len);
    cc_memcpy(entry + HASHMAP_ENTRY_HEADER_SIZE + key->len, val->data, val->len);
}

static inline char *
_entry_val(char *entry)
{
    return entry + HASHMAP_ENTRY_HEADER_SIZE + _entry_klen(entry);
}

static inline char *
_next_entry(char *entry)
{
    return entry + HASHMAP_ENTRY_HEADER_SIZE + _entry_klen(entry) + _entry_vlen(entry);
}


/* returns true if an exact match is found, false otherwise.
 * If a match is found, the position of the entry element is stored in pos;
 * otherwise, pos contains the position of the insertion spot
 */
static inline bool
_locate(char **pos, uint32_t *idx, const char *entry, uint32_t nentry, struct bstring *key)
{
    uint32_t i;
    int bcmp_sgn;
    int eklen_sgn;

    ASSERT(idx != NULL);

    if (nentry == 0) {
        return false;
    }

    for (*pos = entry; *idx < nentry; *idx += 1) {
        uint8_t eklen = _entry_klen(*pos);
        bcmp_sgn = cc_bcmp(key->data, _entry_key(*pos), MIN(key->len, eklen));
        if (bcmp_sgn > 0) { /* no match, insert position found */
            return false;
        }
        if (bcmp_sign < 0) { /* no match, next entry */
            *pos = _next_entry(*pos);
            continue;
        }

        /* bcmp_sign == 0, may need to look at length. A shorter key is smaller */

        /* -1: key is shorter than eklen; 0: equal length; 1: key is longer */
        eklen_sgn = (key->len > eklen) - (eklen > key->len);
        if (eklen_sgn > 0) { /* no match, insert position found */
            return false;
        }
        if (eklen_sgn < 0) { /* no match, next entry */
            *pos = _next_entry(*pos);
            continue;
        }

        return true; /* match iff bcmp_sgn and eklen_sgn are both 0 */
    }

    return false; /* *pos points to the end of body */
}


hashmap_rstatus_e
hashmap_init(hashmap_p hm)
{
    if (hm == NULL) {
        log_debug("NULL pointer encountered for hm");

        return HASHMAP_ERROR;
    }

    HM_NENTRY(hm) = 0;
    HM_NBYTE(hm) = 0;

    return HASHMAP_OK;
}


hashmap_rstatus_e
hashmap_get(struct bstring *val, const hashmap_p hm, const struct bstring *key)
{
    uint32_t idx, nentry;
    char *entry;

    if (key == NULL || val == NULL || hm == NULL) {
        log_debug("NULL pointer encountered for hm %p, key %p, or val %p", hm,
                key, val);

        return HASHMAP_ERROR;
    }

    idx = 0;
    nentry = hashmap_nentry(hm);
    if (_locate(&entry, &idx, HM_BODY(hm), nentry, key)) { /* found */
        val->len = _entry_vlen(entry);
        val->data = entry_val(entry);
        return HASHMAP_OK;
    } else {
        val->len = 0;
        val->data = NULL;
        return HASHMAP_ENOTFOUND;
    }
}


uint32_t
hashmap_multiget(struct bstring *val[], const hashmap_p hm, const struct bstring *key[], uint32_t cardinality)
{
    uint32_t k, idx, nentry, nfound;
    char *entry, *curr;

    if (key == NULL || val == NULL || hm == NULL) {
        log_debug("NULL pointer encountered for hm %p, key %p, or val %p", hm,
                key, val);

        return HASHMAP_ERROR;
    }

    idx = 0;
    nentry = hashmap_nentry(hm);
    for (k = 0, curr = HM_BODY(hm), nfound = 0; k < cardinality; k++) {
        if (_locate(&entry, &idx, curr, nentry, key)) { /* found */
            val[k]->len = _entry_vlen(entry);
            val[k]->data = entry_val(entry);
            nfound++;
        } else {
            val[k]->len = 0;
            val[k]->data = NULL;
        }
    }

    return nfound;
}


hashmap_rstatus_e
hashmap_insert(hashmap_p hm, const struct bstring *key, const struct bstring *val)
{
    bool found;
    char *body, *entry;
    uint32_t idx, nentry;

    if (hm == NULL) {
        log_debug("NULL pointer encountered for hm");

        return HASHMAP_ERROR;
    }

    if (key->len > UINT8_MAX || val->len > UINT8_MAX) {
        log_debug("key / value size too big for current hashmap implementation:"
                "key size: %"PRIu32", val size: %"PRIu32". (Allowed: %"PRIu8")",
                key->len, val->len, UINT8_MAX);

        return HASHMAP_EINVALID;
    }

    body = HM_BODY(hm);
    nentry = hashmap_nentry(hm);

    if (_locate(&entry, &idx, HM_BODY(hm), nentry, key)) { /* found */
        return HASHMAP_EDUP;
    }

    if (entry < HM_END(hm)) {
        cc_memmove(entry + HASHMAP_ENTRY_HEADER_SIZE + key->len + val->len,
                entry, HM_END(hm) - entry);
    }
    _entry_set(entry, key, val);

    HM_NENTRY(hm) += 1;
    HM_NBODY(hm) += _entry_nbyte(entry);

    return HASHMAP_OK;
}

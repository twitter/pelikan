#include "smap.h"

#include <cc_debug.h>


#define SM_BODY(_sm) ((char *)(_sm) + SMAP_HEADER_SIZE)
#define SCAN_THRESHOLD 64

static inline char *
_position(char *body, uint32_t esize, uint32_t idx)
{
    return body + esize * idx;
}


/* false if key is out of range for entry size */
static inline bool
_validate_range(uint16_t ksize, uint64_t key)
{
    switch (ksize) {
    case 8:
        return true;
    case 4:
        return key <= UINT32_MAX;
    case 2:
        return key <= UINT16_MAX;
    case 1:
        return key <= UINT8_MAX;
    default:
        NOT_REACHED();
        return false;
    }
}

static inline uint64_t
_get_key(char *p, uint16_t ksize)
{
    switch (ksize) {
    case 8:
        return *((uint64_t *)p);
    case 4:
        return *((uint32_t *)p);
    case 2:
        return *((uint16_t *)p);
    case 1:
        return *p;
    default:
        NOT_REACHED();
        return 0;
    }
}

static inline void
_set_key(char *p, uint16_t ksize, uint64_t key)
{
    switch (ksize) {
    case 8:
        *((uint64_t *)p) = key;
        break;
    case 4:
        *((uint32_t *)p) = key;
        break;
    case 2:
        *((uint16_t *)p) = key;
        break;
    case 1:
        *p = key;
        break;
    default:
        NOT_REACHED();
    }
}

static inline bool
_should_scan(uint32_t nentry, uint32_t esize) {
    return nentry * esize <= SCAN_THRESHOLD;
}

/* returns true if an exact match is found, false otherwise.
 * If a match is found, the index of the element is stored in idx;
 * otherwise, idx contains the index of the insertion spot
 */
static inline bool
_linear_search(uint32_t *idx, char *body, uint32_t nentry, uint32_t esize,
        uint16_t ksize, uint64_t key)
{
    uint32_t i;

    *idx = 0;

    if (nentry == 0) {
        return false;
    }


    for (i = 0; i < nentry; ++i, ++*idx) {
        if (key <= _get_key(_position(body, esize, i), ksize)) {
            break;
        }
    }

    if (key == _get_key(_position(body, esize, *idx), ksize)) {
        return true;
    } else {
        return false;
    }
}

static inline bool
_binary_search(uint32_t *idx, char *body, uint32_t nentry, uint32_t esize,
        uint16_t ksize, uint64_t key)
{
    uint32_t id = 0, imin, imax;
    uint32_t curr;

    *idx = 0;

    if (nentry == 0) {
        return false;
    }

    if (key == _get_key(_position(body, esize, 0), ksize)) {
        return true;
    }
    if (key < _get_key(_position(body, esize, 0), ksize)) {
        return false;
    }
    if (key > _get_key(_position(body, esize, nentry - 1), ksize)) {
        *idx = nentry;
        return false;
    }

    imin = 1;
    imax = nentry - 1;
    while (imin <= imax) {
        id = (imin + imax) / 2;
        curr = _get_key(_position(body, esize, id), ksize);
        if (key == curr) {
            *idx = id;
            return true;
        }

        if (key > curr) {
            imin = id + 1;
        } else {
            if (key <= _get_key(_position(body, esize, id - 1), ksize)) {
                imax = id - 1;
            } else {
                break;
            }
        }
    }

    *idx = id;

    return false;
}

static inline bool
_locate(uint32_t *idx, char *body, uint32_t nentry, uint32_t esize,
        uint16_t ksize, uint64_t key)
{
    /* optimize for inserting at the end, which is dominant in many use cases */
    if (nentry == 0 || _get_key(body + esize * (nentry - 1), esize) < key) {
        *idx = nentry;

        return false;
    }

    if (_should_scan(nentry, esize)) { /* linear scan */
        return _linear_search(idx, body, nentry, esize, ksize, key);
    } else { /* otherwise, binary search  */
        return _binary_search(idx, body, nentry, esize, ksize, key);
    }
}


smap_rstatus_e
smap_init(smap_p sm, uint16_t ksize, uint16_t vsize)
{
    if (sm == NULL) {
        log_debug("NULL pointer encountered for sm");

        return SMAP_ERROR;
    }

    if (ksize != 1 && ksize != 2 && ksize != 4 && ksize != 8) {
        log_debug("%"PRIu32" is not a valid key size", ksize);

        return SMAP_EINVALID;
    }

    SM_NENTRY(sm) = 0;
    SM_KSIZE(sm) = ksize;
    SM_VSIZE(sm) = vsize;

    return SMAP_OK;
}


smap_rstatus_e
smap_keyval(uint64_t *key, struct bstring *val, const smap_p sm, uint32_t idx)
{
    uint32_t esize, ksize, nentry;
    char *entry;

    if (key == NULL || val == NULL || sm == NULL) {
        log_debug("NULL pointer encountered for sm %p, key %p, or val %p", sm,
                key, val);

        return SMAP_ERROR;
    }

    nentry = smap_nentry(sm);
    idx += (idx < 0) * nentry;
    if (idx < 0 || idx >= nentry) {
        return SMAP_EOOB;
    }

    esize = smap_esize(sm);
    ksize = smap_ksize(sm);
    entry = _position(SM_BODY(sm), esize, idx);

    *key = _get_key(entry, ksize);
    val->len = smap_vsize(sm);
    val->data = entry + ksize;

    return SMAP_OK;
}

/* caller should discard keys in idx if function returns ENOTFOUND */
smap_rstatus_e
smap_index(uint32_t *idx, const smap_p sm, uint64_t key)
{
    uint32_t esize;
    uint16_t ksize;
    bool found;

    if (sm == NULL || idx == NULL) {
        log_debug("NULL pointer encountered for sm %p or key %p", sm, key);

        return SMAP_ERROR;
    }

    ksize = smap_ksize(sm);
    if (!_validate_range(ksize, key)) {
        log_debug("%"PRIu64" out of range for %"PRIu32" byte integer", key,
                ksize);

        return SMAP_EINVALID;
    }

    esize = smap_esize(sm);
    found = _locate(idx, SM_BODY(sm), smap_nentry(sm), esize, ksize, key);
    if (found) {
        return SMAP_OK;
    } else {
        return SMAP_ENOTFOUND;
    }
}


smap_rstatus_e
smap_insert(smap_p sm, uint64_t key, const struct bstring *val)
{
    bool found;
    char *body, *p;
    uint16_t ksize, vsize;
    uint32_t idx, esize, nentry;

    if (sm == NULL) {
        log_debug("NULL pointer encountered for sm");

        return SMAP_ERROR;
    }

    ksize = smap_ksize(sm);
    if (!_validate_range(ksize, key)) {
        log_debug("%"PRIu64" out of range for %"PRIu32" byte integer", key,
                ksize);

        return SMAP_EINVALID;
    }
    vsize = smap_vsize(sm);
    if (val->len != vsize) {
        log_debug("value size %"PRIu16" is different for %"PRIu16" map initial",
                val->len, vsize);

        return SMAP_EINVALID;
    }

    body = SM_BODY(sm);
    nentry = smap_nentry(sm);
    esize = smap_esize(sm);
    found = _locate(&idx, body, nentry, esize, ksize, key);
    if (found) {
        return SMAP_EDUP;
    }

    p = _position(body, esize, idx);
    cc_memmove(p + esize, p, esize * (nentry - idx));
    _set_key(p, ksize, key);
    cc_memcpy(p + ksize, val->data, vsize); /* copy val */
    SM_NENTRY(sm)++;

    return SMAP_OK;
}

smap_rstatus_e
smap_remove(smap_p sm, uint64_t key)
{
    bool found;
    char *body, *p;
    uint16_t ksize;
    uint32_t idx, esize, nentry;

    if (sm == NULL) {
        log_debug("NULL pointer encountered for sm");

        return SMAP_ERROR;
    }

    ksize = smap_ksize(sm);
    if (!_validate_range(ksize, key)) {
        log_debug("%"PRIu64" out of range for %"PRIu32" byte integer", key,
                ksize);

        return SMAP_EINVALID;
    }

    body = SM_BODY(sm);
    nentry = smap_nentry(sm);
    esize = smap_esize(sm);
    found = _locate(&idx, body, nentry, esize, ksize, key);
    if (found) {
        p = _position(body, esize, idx);
        cc_memmove(p, p + esize, esize * (nentry - idx - 1));
        SM_NENTRY(sm)--;

        return SMAP_OK;
    }

    return SMAP_ENOTFOUND;
}

smap_rstatus_e
smap_truncate(smap_p sm, int64_t count)
{
    char *body;
    uint32_t esize, nentry;

    if (sm == NULL) {
        log_debug("NULL pointer encountered for sm");

        return SMAP_ERROR;
    }

    if (count == 0) {
        return SMAP_OK;
    }

    body = SM_BODY(sm);
    esize = smap_esize(sm);
    nentry = smap_nentry(sm);
    /* if abs(count) >= num entries in the array, remove all */
    if (count >= nentry || -count >= nentry) {
        SM_NENTRY(sm) = 0;

        return SMAP_OK;
    }

    if (count > 0) { /* only need to move data if truncating from left */
        cc_memmove(body, body + esize * count, esize * (nentry - count));
        SM_NENTRY(sm) -= count;
    } else {
        SM_NENTRY(sm) += count;
    }

    return SMAP_OK;
}

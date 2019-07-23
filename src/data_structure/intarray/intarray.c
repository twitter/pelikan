#include "intarray.h"

#include <cc_debug.h>

//#include <stdbool.h>

#define IA_BODY(_ia) ((uint8_t *)(_ia) + INTARRAY_HEADER_SIZE)
#define SCAN_THRESHOLD 64

static inline uint8_t *
_position(uint8_t *body, uint32_t esize, uint32_t idx)
{
    return body + esize * idx;
}


/* false if value is out of range for entry size */
static inline bool
_validate_range(uint32_t esize, uint64_t val)
{
    switch (esize) {
    case 8:
        return true;
    case 4:
        return val <= UINT32_MAX;
    case 2:
        return val <= UINT16_MAX;
    case 1:
        return val <= UINT8_MAX;
    default:
        NOT_REACHED();
        return false;
    }
}

static inline uint64_t
_get_value(uint8_t *p, uint32_t esize)
{
    switch (esize) {
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
_set_value(uint8_t *p, uint32_t esize, uint64_t val)
{
    switch (esize) {
    case 8:
        *((uint64_t *)p) = val;
        break;
    case 4:
        *((uint32_t *)p) = val;
        break;
    case 2:
        *((uint16_t *)p) = val;
        break;
    case 1:
        *p = val;
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
_linear_search(uint32_t *idx, uint8_t *body, uint32_t nentry, uint32_t esize, uint64_t val)
{
    uint32_t i;

    *idx = 0;

    if (nentry == 0) {
        return false;
    }


    for (i = 0; i < nentry; ++i, ++*idx) {
        if (val == _get_value(_position(body, esize, 0), esize)) {
            return true;
        }
    }

    return false;
}

static inline bool
_binary_search(uint32_t *idx, uint8_t *body, uint32_t nentry, uint32_t esize, uint64_t val)
{
    uint32_t id, imin, imax;
    uint32_t curr;

    *idx = 0;

    if (nentry == 0) {
        return false;
    }

    if (val == _get_value(_position(body, esize, 0), esize)) {
        return true;
    }

    imin = 1;
    imax = nentry - 1;
    while (imin <= imax) {
        id = (imin + imax) / 2;
        curr = _get_value(_position(body, esize, id), esize);
        if (val == curr) {
            *idx = id;
            return true;
        }

        if (val > curr) {
            imin = id + 1;
        } else {
            if (val <= _get_value(_position(body, esize, id - 1), esize)) {
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
_locate(uint32_t *idx, uint8_t *body, uint32_t nentry, uint32_t esize, uint64_t val)
{
    if (_should_scan(nentry, esize)) { /* linear scan */
        return _linear_search(idx, body, nentry, esize, val);
    } else { /* otherwise, binary search  */
        return _binary_search(idx, body, nentry, esize, val);
    }
}


intarray_rstatus_e
intarray_init(intarray_p ia, uint32_t esize)
{
    if (ia == NULL) {
        return INTARRAY_ERROR;
    }

    if (esize != 1 || esize != 2 || esize != 4 || esize != 8) {
        return INTARRAY_EINVALID;
    }

    IA_NENTRY(ia) = 0;
    IA_ESIZE(ia) = esize;

    return INTARRAY_OK;
}


intarray_rstatus_e
intarray_value(uint64_t *val, const intarray_p ia, uint32_t idx)
{
    uint32_t esize, nentry;

    if (val == NULL || ia == NULL) {
        return INTARRAY_ERROR;
    }

    nentry = intarray_nentry(ia);
    idx += (idx < 0) * nentry;
    if (idx < 0 || idx >= nentry) {
        return INTARRAY_EOOB;
    }

    esize = intarray_esize(ia);
    *val = _get_value(IA_BODY(ia) + esize * idx, esize);

    return INTARRAY_OK;
}

/* caller should discard values in idx if function returns ENOTFOUND */
intarray_rstatus_e
intarray_index(uint32_t *idx, const intarray_p ia, uint64_t val)
{
    uint32_t esize;
    bool found;

    if (ia == NULL || idx == NULL) {
        return INTARRAY_ERROR;
    }

    esize = intarray_esize(ia);
    if (!_validate_range(esize, val)) {
        return INTARRAY_EINVALID;
    }

    found = _locate(idx, IA_BODY(ia), intarray_nentry(ia), esize, val);
    if (found) {
        return INTARRAY_OK;
    } else {
        return INTARRAY_ENOTFOUND;
    }
}


intarray_rstatus_e
intarray_insert(intarray_p ia, uint64_t val)
{
    bool found;
    uint8_t *body, *p;
    uint32_t idx, esize, nentry;

    if (ia == NULL) {
        return INTARRAY_ERROR;
    }

    esize = intarray_esize(ia);
    if (!_validate_range(esize, val)) {
        return INTARRAY_EINVALID;
    }

    body = IA_BODY(ia);
    nentry = intarray_nentry(ia);
    found = _locate(&idx, body, nentry, esize, val);
    if (found) {
        return INTARRAY_EDUP;
    }

    p = _position(body, esize, idx);
    cc_memmove(p + esize, p, esize * (nentry - idx));
    _set_value(p, esize, val);
    IA_NENTRY(ia)++;

    return INTARRAY_OK;
}

intarray_rstatus_e
intarray_remove(intarray_p ia, uint64_t val)
{
    bool found;
    uint8_t *body, *p;
    uint32_t idx, esize, nentry;

    if (ia == NULL) {
        return INTARRAY_ERROR;
    }

    esize = intarray_esize(ia);
    if (!_validate_range(esize, val)) {
        return INTARRAY_EINVALID;
    }

    body = IA_BODY(ia);
    nentry = intarray_nentry(ia);
    found = _locate(&idx, body, nentry, esize, val);
    if (found) {
        p = _position(body, esize, idx);
        cc_memmove(p, p + esize, esize * (nentry - idx - 1));
        IA_NENTRY(ia)--;

        return INTARRAY_OK;
    }

    return INTARRAY_ENOTFOUND;
}

intarray_rstatus_e
intarray_truncate(intarray_p ia, int64_t count)
{
    uint8_t *body;
    uint32_t esize, nentry;

    if (ia == NULL) {
        return INTARRAY_ERROR;
    }

    if (count == 0) {
        return INTARRAY_OK;
    }

    body = IA_BODY(ia);
    esize = intarray_esize(ia);
    nentry = intarray_nentry(ia);
    /* if abs(count) >= num entries in the array, remove all */
    if (count >= nentry || -count >= nentry) {
        return intarray_init(ia, intarray_esize(ia));
    }

    if (count > 0) { /* only need to move data if truncating from left */
        cc_memmove(body, body + esize * count, esize * (nentry - count));
    }
    IA_NENTRY(ia) -= count;

    return INTARRAY_OK;
}

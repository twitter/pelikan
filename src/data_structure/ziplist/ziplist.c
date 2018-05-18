#include "ziplist.h"

#include <cc_debug.h>

static uint8_t ze_buf[UINT8_MAX];


static inline uint8_t *
_ziplist_end(ziplist_p zl)
{
    return zl + ZL_NEND(zl);
}

/* zipentry size required for a value */
static inline uint8_t
_encode_size(const struct blob *val)
{
    if (val->type == BLOB_TYPE_STR) {
        return ZE_ZELEN_LEN + ZE_STR_HEADER + val->vstr.len;
    } else { /* BLOB_TYPE_INT */
        if (val->vint <= ZE_U8_MAX) {
            return ZE_ZELEN_LEN + ZE_U8_LEN;
        } else if (val->vint <= ZE_U16_MAX) {
            return ZE_ZELEN_LEN + ZE_U16_LEN;
        } else if (val->vint <= ZE_U24_MAX) {
            return ZE_ZELEN_LEN + ZE_U24_LEN;
        } else if (val->vint <= ZE_U56_MAX) {
            return ZE_ZELEN_LEN + ZE_U56_LEN;
        } else { /* ZE_U64_MAX */
            return ZE_ZELEN_LEN + ZE_U64_LEN;
        }
    }
}

static inline uint8_t
_zipentry_len(zipentry_p ze)
{
    if (*ze <= ZE_U8_MAX) {
        return ZE_ZELEN_LEN + ZE_U8_LEN;
    } else if (*ze == ZE_U16) {
        return ZE_ZELEN_LEN + ZE_U16_LEN;
    } else if (*ze == ZE_U24) {
        return ZE_ZELEN_LEN + ZE_U24_LEN;
    } else if (*ze == ZE_U56) {
        return ZE_ZELEN_LEN + ZE_U56_LEN;
    } else if (*ze == ZE_U64) {
        return ZE_ZELEN_LEN + ZE_U64_LEN;
    } else { /* ZE_STR */
        return ZE_ZELEN_LEN + ZE_STR_HEADER + *(ze + 1);
    }
}

static inline uint64_t
_zipentry_int(zipentry_p ze) {
    if (*ze <= ZE_U8_MAX) {
        return *((uint8_t*)ze);
    } else if (*ze == ZE_U16) {
        return *((uint16_t*)(ze + 1));
    } else if (*ze == ZE_U24) {
        return *((uint32_t*)ze) >> 8;
    } else if (*ze == ZE_U56) {
        return *((uint64_t*)ze) >> 8;
    } else if (*ze == ZE_U64) {
        return *((uint64_t*)(ze + 1));
    } else {
        NOT_REACHED();
        return ZE_U64_MAX;
    }
}

static inline struct bstring
_zipentry_str(zipentry_p ze) {
    ASSERT(*ze == ZE_STR);

    /* .len, .data */
    return (struct bstring){*(ze + 1), (char *)ze + 2};
}

ziplist_rstatus_e
zipentry_size(uint8_t *sz, struct blob *val)
{
    if (val->type == BLOB_TYPE_UNKNOWN || val->type >= BLOB_TYPE_SENTINEL ||
            (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN)) {
        return ZIPLIST_EINVALID;
    }

    *sz = _encode_size(val);

    return ZIPLIST_OK;
}

int
zipentry_compare(const zipentry_p ze, const struct blob *val)
{
    ASSERT(ze != NULL && val != NULL);

    struct blob zev;

    zipentry_get(&zev, ze);

    return blob_compare(&zev, val);
}

ziplist_rstatus_e
zipentry_get(struct blob *val, const zipentry_p ze)
{
    if (ze == NULL) {
        return ZIPLIST_ERROR;
    }

    if (*ze == ZE_STR) {
        val->type = BLOB_TYPE_STR;
        val->vstr = _zipentry_str(ze);
    } else {
        val->type = BLOB_TYPE_INT;
        val->vint = _zipentry_int(ze);
    }

    return ZIPLIST_OK;
}

static inline uint32_t
_zipentry_encode(zipentry_p ze, const struct blob *val)
{
    uint8_t len = _encode_size(val);

    if (val->type == BLOB_TYPE_STR) {
        *ze = ZE_STR;
        *(ze + 1) = val->vstr.len;
        cc_memcpy(ze + 2, val->vstr.data, val->vstr.len);
    } else {
        if (val->vint <= ZE_U8_MAX) {
            *ze = val->vint;
        } else if (val->vint <= ZE_U16_MAX) {
            *ze = ZE_U16;
            *((uint16_t*)(ze + 1)) = (uint16_t)val->vint;
        } else if (val->vint <= ZE_U24_MAX) {
            *((uint32_t*)ze) = (uint32_t)((val->vint << 8) + ZE_U24);
        } else if (val->vint <= ZE_U56_MAX) {
            *((uint64_t*)ze) = (uint64_t)((val->vint << 8) + ZE_U56);
        } else { /* 64-bit uint */
            *ze = ZE_U64;
            *((uint64_t*)(ze + 1)) = val->vint;
        }
    }

    *(ze + len - 1) = len; /* set len at the end */

    return len;
}

ziplist_rstatus_e
zipentry_set(zipentry_p ze, const struct blob *val)
{
    if (ze == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_UNKNOWN || val->type >= BLOB_TYPE_SENTINEL ||
            (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN)) {
        return ZIPLIST_EINVALID;
    }

    _zipentry_encode(ze, val);

    return ZIPLIST_OK;
}


static inline zipentry_p
_ziplist_head(ziplist_p zl)
{
    return zl + ZIPLIST_HEADER_SIZE;
}

static inline zipentry_p
_ziplist_tail(ziplist_p zl)
{
    uint8_t *p = _ziplist_end(zl);

    return p - *p + 1;
}

ziplist_rstatus_e
ziplist_reset(ziplist_p zl)
{
    if (zl == NULL) {
        return ZIPLIST_ERROR;
    }

    ZL_NENTRY(zl) = 0;
    ZL_NEND(zl) = ZIPLIST_HEADER_SIZE - 1;

    return ZIPLIST_OK;
}

/* do NOT call this function on the first zip entry, use ziplist_prev */
static inline zipentry_p
_ziplist_prev(zipentry_p ze)
{
    return ze - *(ze - 1); /* *(ze - 1) : length of the previous entry */
}

/* do NOT call this function on the last zip entry, use ziplist_prev */
static inline zipentry_p
_ziplist_next(zipentry_p ze)
{
    return ze + _zipentry_len(ze);
}

ziplist_rstatus_e
ziplist_prev(zipentry_p *ze, ziplist_p zl, const zipentry_p curr)
{
    if (curr == _ziplist_head(zl)) {
        return ZIPLIST_EOOB;
    } else {
        *ze = _ziplist_prev(curr);
        return ZIPLIST_OK;
    }
}

ziplist_rstatus_e
ziplist_next(zipentry_p *ze, ziplist_p zl, const zipentry_p curr)
{
    if (curr == _ziplist_tail(zl)) {
        return ZIPLIST_EOOB;
    } else {
        *ze = _ziplist_next(curr);
        return ZIPLIST_OK;
    }
}

static inline zipentry_p
_ziplist_fromleft(const ziplist_p zl, uint32_t idx)
{
    zipentry_p ze = _ziplist_head(zl);

    for (; idx > 0; idx--, ze += _zipentry_len(ze));

    return ze;
}

static inline zipentry_p
_ziplist_fromright(const ziplist_p zl, uint32_t idx)
{
    uint8_t *p = _ziplist_end(zl);

    for (; idx > 0; idx--, p -= *p);

    return p - *p + 1;
}

ziplist_rstatus_e
ziplist_locate(zipentry_p *ze, const ziplist_p zl, int64_t idx)
{
    uint32_t nentry;

    if (zl == NULL || ze == NULL) {
        return ZIPLIST_ERROR;
    }

    nentry = ziplist_nentry(zl);
    idx += (idx < 0) * nentry;
    if (idx < 0 || idx >= nentry) {
        *ze = NULL;
        return ZIPLIST_EOOB;
    }

    /* suspecting it's generally cheaper to jump backwards due to encoding,
     * the cutoff is unclear until we benchmark it, so the number chosen here
     * is arbitrary for now
     */
    if (idx * 3 < nentry) {
        *ze = _ziplist_fromleft(zl, idx);
    } else {
        *ze = _ziplist_fromright(zl, nentry - 1 - idx);
    }

    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_find(zipentry_p *ze, int64_t *idx, const ziplist_p zl, const struct blob *val)
{
    uint32_t nentry, len;
    int64_t i;
    zipentry_p z;
    uint8_t *end;

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_UNKNOWN || val->type >= BLOB_TYPE_SENTINEL ||
            (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN)) {
        return ZIPLIST_EINVALID;
    }

    nentry = ziplist_nentry(zl);
    /* Encoding one struct blob and follow up with many simple memcmp should be
     * faster than decoding each of the zentries being compared.
     */
    len = _zipentry_encode(ze_buf, val);
    end = _ziplist_end(zl);
    for (i = 0, z =_ziplist_head(zl); i < nentry; i++, z = _ziplist_next(z)) {
        if (memcmp(z, ze_buf, MIN(end - z + 1, len)) == 0) { /* found */
            if (idx != NULL) {
                *idx = i;
            }
            if (ze != NULL) {
                *ze = z;
            }

            return ZIPLIST_OK;
        }
    }

    /* not found */
    if (idx != NULL) {
        *idx = -1;
    }
    if (ze != NULL) {
        *ze = NULL;
    }
    return ZIPLIST_ENOTFOUND;
}

static inline void
_ziplist_remove(ziplist_p zl, zipentry_p begin, zipentry_p end, uint32_t count)
{
    cc_memmove(begin, end, _ziplist_end(zl) + 1 - end);

    ZL_NENTRY(zl) -= count;
    ZL_NEND(zl) -= (uint32_t)(end - begin);
}

ziplist_rstatus_e
ziplist_remove_val(uint32_t *removed, ziplist_p zl, const struct blob *val,
        int64_t count)
{
    uint32_t len, atmost;
    int64_t i = 0;
    zipentry_p z;
    uint8_t *end;
    bool forward = (count > 0);

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_UNKNOWN || val->type >= BLOB_TYPE_SENTINEL ||
            (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN)) {
        return ZIPLIST_EINVALID;
    }

    if (count == 0) {
        return ZIPLIST_EINVALID;
    }

    atmost = forward ? count : -count;
    *removed = 0;

    /* Encoding one struct blob and follow up with many simple memcmp should be
     * faster than decoding each of the zentries being compared.
     */
    len = _zipentry_encode(ze_buf, val);

    z = forward ? _ziplist_head(zl) : _ziplist_tail(zl);
    for (; i < atmost; ++i) {
        /* find next */
        end = _ziplist_end(zl);
        while (memcmp(z, ze_buf, MIN(end - z + 1, len)) != 0) {
            if (forward) {
                if (z == _ziplist_tail(zl)) {
                    return ZIPLIST_OK;
                }
                z = _ziplist_next(z);
            } else {
                if (z == _ziplist_head(zl)) {
                    return ZIPLIST_OK;
                }
                z = _ziplist_prev(z);
            }
        }

        _ziplist_remove(zl, z, _ziplist_next(z), 1);
        *removed += 1;
    }

    return ZIPLIST_OK;
}


ziplist_rstatus_e
ziplist_remove(ziplist_p zl, int64_t idx, int64_t count)
{
    uint32_t nentry, i = 0;
    zipentry_p begin, end;

    if (zl == NULL) {
        return ZIPLIST_ERROR;
    }

    if (count == 0) {
        return ZIPLIST_EINVALID;
    }

    nentry = ziplist_nentry(zl);
    idx += (idx < 0) * nentry;
    if (count < 0) { /* counting backward, move idx back */
        count = -count;
        idx = idx - count + 1;
    }
    if (idx < 0 || idx > nentry || idx + count - 1 >= nentry) {
        return ZIPLIST_EOOB;
    }

    ziplist_locate(&begin, zl, idx);
    for (end = begin; i < count; ++i, end += _zipentry_len(end));

    _ziplist_remove(zl, begin, end, (uint32_t)count);

    return ZIPLIST_OK;
}

static inline void
_ziplist_add(ziplist_p zl, zipentry_p ze, struct blob *val)
{
    uint8_t sz;

    sz = _zipentry_encode(ze, val);

    ZL_NENTRY(zl) += 1;
    ZL_NEND(zl) += sz;
}

ziplist_rstatus_e
ziplist_insert(ziplist_p zl, struct blob *val, int64_t idx)
{
    uint32_t nentry;
    uint8_t sz;
    zipentry_p ze;

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_UNKNOWN || val->type >= BLOB_TYPE_SENTINEL ||
            (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN)) {
        return ZIPLIST_EINVALID;
    }

    nentry = ziplist_nentry(zl);
    idx += (idx < 0) * nentry;
    if (idx < 0 || idx > nentry) {
        return ZIPLIST_EOOB;
    }

    if (idx == nentry) {
        ze = _ziplist_end(zl) + 1;
    } else {
        sz = _encode_size(val);
        ziplist_locate(&ze, zl, idx);
        /* right shift */
        cc_memmove(ze + sz, ze, _ziplist_end(zl) - ze + 1);
    }

    _ziplist_add(zl, ze, val);

    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_push(ziplist_p zl, struct blob *val)
{
    zipentry_p ze;

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_UNKNOWN || val->type >= BLOB_TYPE_SENTINEL ||
            (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN)) {
        return ZIPLIST_EINVALID;
    }

    ze = _ziplist_end(zl) + 1;
    _ziplist_add(zl, ze, val);

    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_pop(struct blob *val, ziplist_p zl)
{
    zipentry_p ze;
    uint8_t *end;

    if (zl == NULL) {
        return ZIPLIST_ERROR;
    }

    if (ziplist_nentry(zl) == 0) {
        return ZIPLIST_EOOB;
    }

    ze = _ziplist_fromright(zl, 0);
    end = _ziplist_end(zl);
    if (val != NULL) {
        zipentry_get(val, ze); /* won't fail */
    }
    ZL_NENTRY(zl) -= 1;
    ZL_NEND(zl) -= *end;

    return ZIPLIST_OK;
}

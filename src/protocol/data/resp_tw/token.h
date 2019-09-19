#pragma once

/*
 * This file handles serialization and deserialization for the
 * RESP3 format.
 */

/*
 * Functions that deal with tokens in RESP3. 
 * 
 * In RESP3 the type of a value is decided by its leading character.
 *   - Blob strings start with '$'
 *   - Simple strings start with '+'
 *   - Simple errors start with '-'
 *   - Numbers start with ':'
 *   - Nil starts with '_'
 *   - Doubles start with ','
 *   - Booleans start with '#'
 *   - Blob errors start with '!'
 *   - Verbatim strings start with '='
 *   - Big numbers start with '('
 *   - Arrays start with '*'
 *   - Maps start with '%'
 *   - Sets start with '~'
 *   - Attributes start with '|'
 *   - Push data starts with '>'
 * 
 */

#include <stdbool.h>

#include <buffer/cc_buf.h>
#include <cc_bstring.h>
#include <cc_util.h>

/* Note: most of these enums are identical to the ones in the resp folder */

typedef enum parse_rstatus {
    PARSE_OK            = 0,
    PARSE_EUNFIN        = -1,
    PARSE_EEMPTY        = -2,
    PARSE_EOVERSIZE     = -3,
    PARSE_EINVALID      = -4,
    PARSE_EOTHER        = -5,
    PARSE_ENOTSUPPORTED = -6,
} parse_rstatus_e;

typedef enum compose_rstatus {
    COMPOSE_OK              = 0,
    COMPOSE_EUNFIN          = -1,
    COMPOSE_ENOMEM          = -2,
    COMPOSE_EINVALID        = -3,
    COMPOSE_EOTHER          = -4,
    COMPOSE_ENOTSUPPORTED   = -5
} compose_rstatus_e;

/* array, map, set, attributes, and push data are not basic element types */
typedef enum element_type {
    ELEM_UNKNOWN        = 0,
    ELEM_STR            = 1,
    ELEM_ERR            = 2,
    ELEM_BLOB_STR       = 3,
    ELEM_BLOB_ERR       = 4,
    ELEM_NUMBER         = 5,
    ELEM_DOUBLE         = 6,  /* Note: currently unsupported */
    ELEM_BOOL           = 7,
    ELEM_VERBATIM_STR   = 8,
    ELEM_BIG_NUMBER     = 9,  /* Note: currently unsupported */
    ELEM_NIL            = 10,
    ELEM_ARRAY          = 11,
    ELEM_MAP            = 12,
    ELEM_SET            = 13,
    ELEM_ATTRIBUTES     = 14,
    ELEM_PUSH_DATA      = 15,
} element_type_e;

struct element {
    element_type_e      type;
    union {
        struct bstring  bstr;
        int64_t         num;
        double          double_;
        bool            boolean;
    };
};

static inline bool
is_crlf(struct buf *buf)
{
    ASSERT(buf_rsize(buf) >= CRLF_LEN);

    return (*buf->rpos == CR && *(buf->rpos + 1) == LF);
}

static inline bool
line_end(struct buf *buf)
{
    return (buf_rsize(buf) >= CRLF_LEN && is_crlf(buf));
}

bool token_is_array(struct buf *buf);
bool token_is_map(struct buf *buf);
bool token_is_set(struct buf *buf);
bool token_is_attribute(struct buf *buf);
bool token_is_push_data(struct buf *buf);

parse_rstatus_e token_array_nelem(uint64_t *nelem, struct buf *buf);
parse_rstatus_e token_set_nelem(uint64_t *nelem, struct buf *buf);
parse_rstatus_e token_map_nelem(uint64_t *nelem, struct buf *buf);
parse_rstatus_e token_attribute_nelem(uint64_t *nelem, struct buf *buf);
parse_rstatus_e token_push_data_nelem(uint64_t *nelem, struct buf *buf);

parse_rstatus_e parse_element(struct element *el, struct buf *buf);

/* Write a type out to the buffer, returns the number of
 * bytes written. Negative values are parse_rstatus_e error
 * codes.
 */
int compose_array_header(struct buf **buf, uint64_t nelem);
int compose_map_header(struct buf **buf, uint64_t nelem);
int compose_set_header(struct buf **buf, uint64_t nelem);
int compose_attribute_header(struct buf **buf, uint64_t nelem);
int compose_push_data_header(struct buf **buf, uint64_t nelem);

int compose_element(struct buf **buf, struct element *el);



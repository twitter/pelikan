#pragma once

/*
 * this file handles the serialization / desrialization formats used by Redis:
 *  - RESP (REdis Serialization Protocol)
 *  - Simple format (not implemented yet)
 *  - Cap'n'proto (not implemented yet)
 */

/**
 * functions that deal with tokens in RESP (REdis Serialization Protocol).
 * RESP is text-based protocol that uses special characters and prefixed-length
 * to achieve high-performance parsing.
 *
 * RESP has the following guidelines for requests/responses:
 *   - Clients send commands to a Redis server as a RESP Array of Bulk Strings.
 *   - The server replies with one of the RESP types according to the command
 *     implementation.
 *
 * Different types have different leading character
 *   - For Simple Strings the first byte of the reply is "+"
 *   - For Errors the first byte of the reply is "-"
 *   - For Integers the first byte of the reply is ":"
 *   - For Bulk Strings the first byte of the reply is "$"
 *   - For Arrays the first byte of the reply is "*"
 *
 * Note:
 *   - In RESP, tokens of each type are always terminated with "\r\n" (CRLF).
 *   - There are multiple ways of representing Null values:
 *     + Null Bulk String: "$-1\r\n"
 *     + Null Array: "*-1\r\n"
 */

/**
 * we add the following type to both RESP request and response, referencing
 * RESP3 protocol (https://github.com/antirez/RESP3/blob/master/spec.md),
 * but not wholesale implementing it):
 *
 *   - Attribute: the first line starts with '|', followed the number of
 *       attributes (A). After the first line, there should be 2xA lines
 *       describing A attributes in key/value (or dictionary) style.
 *       For now, we limit the elements to be of two simple types (Simple
 *       String, Integers). NOTE: different from what's in RESP, this type can
 *       be used in both request and reply, and for now can only be used to
 *       decorate at the top level of the request / response.
 *   - Null: "_\r\n", we keep Null Bulk and Null Array for compatibility, but
 *       all other types should use the new Null.
 */

/**
 * It makes sense to always parse Simple Strings, Errors, and Integers in
 * full. However, for Bulk Strings and Arrays, it is possible that they
 * will be big enough that we cannot always expect the full content to be
 * received at once, and hence it makes sense to allow partial parsing.
 *
 * For Bulk Strings, there are always two tokens, 1) the length; and 2) the
 * string content. Since the content can be quite large, we should remember
 * how many bytes have been received and how many more to expect.
 *
 * Array is a composite type, where individual elements can be any of the other
 * type, and different types can mix in a single array. So to parse an array,
 * we need to handle both a subset of all elements and incompleteness of the
 * last element.
 */

#include <buffer/cc_buf.h>
#include <cc_bstring.h>
#include <cc_util.h>

typedef enum parse_rstatus {
    PARSE_OK        = 0,
    PARSE_EUNFIN    = -1,
    PARSE_EEMPTY    = -2,
    PARSE_EOVERSIZE = -3,
    PARSE_EINVALID  = -4,
    PARSE_EOTHER    = -5,
} parse_rstatus_e;

typedef enum compose_rstatus {
    COMPOSE_OK          = 0,
    COMPOSE_EUNFIN      = -1,
    COMPOSE_ENOMEM      = -2,
    COMPOSE_EINVALID    = -3,
    COMPOSE_EOTHER      = -4,
} compose_rstatus_e;

/* array is not a basic element type */
typedef enum element_type {
    ELEM_UNKNOWN    = 0,
    ELEM_STR        = 1,
    ELEM_ERR        = 2,
    ELEM_INT        = 3,
    ELEM_BULK       = 4,
    ELEM_ARRAY      = 5,
    ELEM_ATTRIB     = 6,
    ELEM_NIL        = 7, /* nil bulk */
    ELEM_NULL       = 8,
} element_type_e;

struct element {
    element_type_e      type;
    union {
        struct bstring  bstr;
        int64_t         num;
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
bool token_is_attrib(struct buf *buf);
parse_rstatus_e parse_element(struct element *el, struct buf *buf);

int compose_element(struct buf **buf, struct element *el);

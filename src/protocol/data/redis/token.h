#pragma once

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

#include "parse.h"
#include "compose.h"

#include <buffer/cc_buf.h>
#include <cc_bstring.h>

/* array is not a basic element type */
typedef enum element_type {
    ELEM_UNKNOWN    = 0,
    ELEM_STR        = 1,
    ELEM_ERR        = 2,
    ELEM_INT        = 3,
    ELEM_BULK       = 4,
} element_type_t;

struct element {
    element_type_t      type;
    union {
        struct bstring  str;
        int64_t         num;
    };
};

bool token_is_array(struct buf *buf);
parse_rstatus_t token_array_nelem(int32_t *nelem, struct buf *buf);

parse_rstatus_t parse_element(struct element *el, struct buf *buf);
int compose_element(struct buf **buf, struct element *el);

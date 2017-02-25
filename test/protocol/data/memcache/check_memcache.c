#include <protocol/data/memcache_include.h>

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_define.h>

#include <check.h>

#include <stdio.h>
#include <stdlib.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "memcache"
#define DEBUG_LOG  SUITE_NAME ".log"

struct request *req;
struct response *rsp;
struct buf *buf;

/*
 * utilities
 */
static void
test_setup(void)
{
    req = request_create();
    rsp = response_create();
    buf = buf_create();
}

static void
test_reset(void)
{
    request_reset(req);
    response_reset(rsp);
    buf_reset(buf);
}

static void
test_teardown(void)
{
    buf_destroy(&buf);
    response_destroy(&rsp);
    request_destroy(&req);
}

/**************
 * test cases *
 **************/

/*
 * basic requests
 */
START_TEST(test_quit)
{
#define SERIALIZED "quit\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    req->type = REQ_QUIT;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_QUIT);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_delete)
{
#define SERIALIZED "delete foo\r\n"
#define KEY "foo"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_DELETE;
    pos = array_push(req->keys);
    *pos = key;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_DELETE);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_delete_noreply)
{
#define SERIALIZED "delete foo noreply\r\n"
#define KEY "foo"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_DELETE;
    pos = array_push(req->keys);
    *pos = key;
    req->noreply = 1;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_DELETE);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->noreply, 1);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_get)
{
#define SERIALIZED "get foo\r\n"
#define KEY "foo"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_GET;
    pos = array_push(req->keys);
    *pos = key;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_GET);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_multikey)
{
#define SERIALIZED "get foo bar baz\r\n"
#define KEY0 "foo"
#define KEY1 "bar"
#define KEY2 "baz"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key0 = str2bstr(KEY0);
    struct bstring key1 = str2bstr(KEY1);
    struct bstring key2 = str2bstr(KEY2);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_GET;
    pos = array_push(req->keys);
    *pos = key0;
    pos = array_push(req->keys);
    *pos = key1;
    pos = array_push(req->keys);
    *pos = key2;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(array_nelem(req->keys), 3);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert_msg(ret == PARSE_OK, "ret: %d", ret);
    ck_assert(req->type == REQ_GET);
    ck_assert_int_eq(array_nelem(req->keys), 3);
    ck_assert_int_eq(bstring_compare(&key0, array_get(req->keys, 0)), 0);
    ck_assert_int_eq(bstring_compare(&key1, array_get(req->keys, 1)), 0);
    ck_assert_int_eq(bstring_compare(&key2, array_get(req->keys, 2)), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY0
#undef KEY1
#undef KEY2
#undef SERIALIZED
}
END_TEST

START_TEST(test_gets)
{
#define SERIALIZED "gets foo\r\n"
#define KEY "foo"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_GETS;
    pos = array_push(req->keys);
    *pos = key;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_GETS);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_set)
{
#define SERIALIZED "set foo 123 86400 3\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123
#define EXPIRY 86400

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_SET;
    pos = array_push(req->keys);
    *pos = key;
    req->flag = FLAG;
    req->expiry = EXPIRY;
    req->vstr = val;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_SET);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->flag, FLAG);
    ck_assert_int_eq(req->expiry, EXPIRY);
    ck_assert_int_eq(bstring_compare(&val, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef EXPIRY
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_partial_value)
{
#define SERIALIZED_1 "set foo 0 0 7\r\nXYZ"
#define SERIALIZED_2 "abcd"
#define SERIALIZED_3 "\r\n"
#define KEY "foo"
#define VAL1 "XYZ"
#define VAL2 "abcd"

    int ret;
    struct bstring key = str2bstr(KEY);
    struct bstring val1 = str2bstr(VAL1);
    struct bstring val2 = str2bstr(VAL2);
    char *pos;

    test_reset();

    /* first segment */
    buf_write(buf, SERIALIZED_1, sizeof(SERIALIZED_1) - 1);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARTIAL);
    ck_assert(req->partial);
    ck_assert(req->nremain == val2.len);
    ck_assert_int_eq(bstring_compare(&val1, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);

    /* second segment */
    buf_lshift(buf);
    buf_write(buf, SERIALIZED_2, sizeof(SERIALIZED_2) - 1);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARTIAL);
    ck_assert(req->partial);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert(req->nremain == 0);
    ck_assert_int_eq(bstring_compare(&val2, &req->vstr), 0);

    /* final segment */
    buf_lshift(buf);
    buf_write(buf, SERIALIZED_3, sizeof(SERIALIZED_3) - 1);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(!req->partial);
    ck_assert(req->nremain == 0);
    ck_assert_int_eq(bstring_compare(&null_bstring, &req->vstr), 0);

    /* if request is not left-shifted, should return EUNFIN */
    pos = buf->rpos;
    request_reset(req);
    buf_write(buf, SERIALIZED_1, sizeof(SERIALIZED_1) - 1);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_EUNFIN);
    ck_assert(buf->rpos == pos);

    /* leftshift, should work now */
    buf_lshift(buf);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);

#undef VAL2
#undef VAL1
#undef KEY
#undef SERIALIZED_3
#undef SERIALIZED_2
#undef SERIALIZED_1
}
END_TEST

START_TEST(test_add_noreply)
{
#define SERIALIZED "add foo 123 86400 3 noreply\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123
#define EXPIRY 86400

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_ADD;
    pos = array_push(req->keys);
    *pos = key;
    req->flag = FLAG;
    req->expiry = EXPIRY;
    req->noreply = 1;
    req->vstr = val;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_ADD);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->flag, FLAG);
    ck_assert_int_eq(req->expiry, EXPIRY);
    ck_assert_int_eq(req->noreply, 1);
    ck_assert_int_eq(bstring_compare(&val, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef EXPIRY
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_replace_noreply)
{
#define SERIALIZED "replace foo 123 86400 3 noreply\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123
#define EXPIRY 86400

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_REPLACE;
    pos = array_push(req->keys);
    *pos = key;
    req->flag = FLAG;
    req->expiry = EXPIRY;
    req->noreply = 1;
    req->vstr = val;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_REPLACE);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->flag, FLAG);
    ck_assert_int_eq(req->expiry, EXPIRY);
    ck_assert_int_eq(req->noreply, 1);
    ck_assert_int_eq(bstring_compare(&val, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef EXPIRY
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_cas)
{
#define SERIALIZED "cas foo 123 86400 3 45678\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123
#define EXPIRY 86400
#define CAS 45678

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_CAS;
    pos = array_push(req->keys);
    *pos = key;
    req->flag = FLAG;
    req->expiry = EXPIRY;
    req->vcas = CAS;
    req->vstr = val;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_CAS);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->flag, FLAG);
    ck_assert_int_eq(req->expiry, EXPIRY);
    ck_assert_int_eq(req->vcas, CAS);
    ck_assert_int_eq(bstring_compare(&val, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef EXPIRY
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_append)
{
#define SERIALIZED "append foo 123 86400 3\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123
#define EXPIRY 86400

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_APPEND;
    pos = array_push(req->keys);
    *pos = key;
    req->flag = FLAG;
    req->expiry = EXPIRY;
    req->vstr = val;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_APPEND);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->flag, FLAG);
    ck_assert_int_eq(req->expiry, EXPIRY);
    ck_assert_int_eq(bstring_compare(&val, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef EXPIRY
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_prepend_noreply)
{
#define SERIALIZED "prepend foo 123 86400 3 noreply\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123
#define EXPIRY 86400

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_PREPEND;
    pos = array_push(req->keys);
    *pos = key;
    req->flag = FLAG;
    req->expiry = EXPIRY;
    req->noreply = 1;
    req->vstr = val;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_PREPEND);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->flag, FLAG);
    ck_assert_int_eq(req->expiry, EXPIRY);
    ck_assert_int_eq(req->noreply, 1);
    ck_assert_int_eq(bstring_compare(&val, &req->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef EXPIRY
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_incr)
{
#define SERIALIZED "incr foo 909\r\n"
#define KEY "foo"
#define DELTA 909

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_INCR;
    pos = array_push(req->keys);
    *pos = key;
    req->delta = DELTA;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_INCR);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->delta, DELTA);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_decr_noreply)
{
#define SERIALIZED "decr foo 909 noreply\r\n"
#define KEY "foo"
#define DELTA 909

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring *pos;

    test_reset();

    /* compose */
    req->type = REQ_DECR;
    pos = array_push(req->keys);
    *pos = key;
    req->delta = DELTA;
    req->noreply = 1;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_DECR);
    ck_assert_int_eq(array_nelem(req->keys), 1);
    ck_assert_int_eq(bstring_compare(&key, array_first(req->keys)), 0);
    ck_assert_int_eq(req->delta, DELTA);
    ck_assert_int_eq(req->noreply, 1);
    ck_assert(buf->rpos == buf->wpos);
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_partial_header)
{
#define SERIALIZED "set foo 123 "

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    char *pos;

    test_reset();

    /* compose */
    buf_write(buf, SERIALIZED, len);
    pos = buf->rpos;

    /* parse (nothing should change) */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_EUNFIN);
    ck_assert(req->rstate == REQ_PARSING);
    ck_assert(buf->rpos == pos);

#undef SERIALIZED
}
END_TEST
/*
 * basic responses
 */
START_TEST(test_ok)
{
#define SERIALIZED "OK\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_OK;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_OK);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_end)
{
#define SERIALIZED "END\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_END;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_END);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_stored)
{
#define SERIALIZED "STORED\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_STORED;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_STORED);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_exists)
{
#define SERIALIZED "EXISTS\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_EXISTS;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_EXISTS);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_deleted)
{
#define SERIALIZED "DELETED\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_DELETED;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_DELETED);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_notfound)
{
#define SERIALIZED "NOT_FOUND\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_NOT_FOUND;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_NOT_FOUND);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

START_TEST(test_notstored)
{
#define SERIALIZED "NOT_STORED\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_NOT_STORED;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_NOT_STORED);
    ck_assert(buf->rpos == buf->wpos);
#undef SERIALIZED
}
END_TEST

/* TODO: move this to the admin test suite */

START_TEST(test_stat)
{
#define SERIALIZED "STAT memory_curr 24642\r\n"
#define NAME "memory_curr"
#define METRIC 24642
//
//    int ret;
//    int len = sizeof(SERIALIZED) - 1;
//    struct metric met = {.name = NAME, .type = METRIC_GAUGE, .gauge = METRIC};
//
//    test_reset();
//
//    /* compose */
//    rsp->type = RSP_STAT;
//    rsp->met = &met;
//    ret = compose_rsp(&buf, rsp);
//    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
//    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);
//
//    /* parse */
//    response_reset(rsp);
//    ret = parse_rsp(rsp, buf);
//    ck_assert_int_eq(ret, PARSE_OK);
//    ck_assert(rsp->rstate == RSP_PARSED);
//    ck_assert(rsp->type == RSP_STAT);
//    ck_assert_int_eq(bstring_compare(&rsp->key, &str2bstr(NAME)), 0);
//    ck_assert_int_eq(rsp->vint, METRIC);
//    ck_assert(buf->rpos == buf->wpos);
#undef METRIC
#undef NAME
#undef SERIALIZED
}
END_TEST

START_TEST(test_value)
{
#define SERIALIZED "VALUE foo 123 3\r\nXYZ\r\n"
#define KEY "foo"
#define VAL "XYZ"
#define FLAG 123

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring key = str2bstr(KEY);
    struct bstring val = str2bstr(VAL);

    test_reset();

    /* compose */
    rsp->type = RSP_VALUE;
    rsp->key = key;
    rsp->vstr = val;
    rsp->flag = FLAG;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_VALUE);
    ck_assert_int_eq(bstring_compare(&rsp->key, &str2bstr(KEY)), 0);
    ck_assert_int_eq(rsp->flag, FLAG);
    ck_assert_int_eq(bstring_compare(&val, &rsp->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef FLAG
#undef VAL
#undef KEY
#undef SERIALIZED
}
END_TEST

START_TEST(test_numeric)
{
#define SERIALIZED "9223372036854775807\r\n"
#define VINT 9223372036854775807

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    rsp->type = RSP_NUMERIC;
    rsp->vint = VINT;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_NUMERIC);
    ck_assert_int_eq(rsp->vint, VINT);
    ck_assert(buf->rpos == buf->wpos);
#undef VINT
#undef SERIALIZED
}
END_TEST

START_TEST(test_servererror)
{
#define SERIALIZED "SERVER_ERROR out of memory\r\n"
#define REASON "out of memory"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring val = str2bstr(REASON);

    test_reset();

    /* compose */
    rsp->type = RSP_SERVER_ERROR;
    rsp->vstr = val;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_SERVER_ERROR);
    ck_assert_int_eq(bstring_compare(&val, &rsp->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef REASON
#undef SERIALIZED
}
END_TEST

START_TEST(test_clienterror)
{
#define SERIALIZED "CLIENT_ERROR oversized value\r\n"
#define REASON "oversized value"

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct bstring val = str2bstr(REASON);

    test_reset();

    /* compose */
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = val;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_CLIENT_ERROR);
    ck_assert_int_eq(bstring_compare(&val, &rsp->vstr), 0);
    ck_assert(buf->rpos == buf->wpos);
#undef REASON
#undef SERIALIZED
}
END_TEST

static void
test_rsp_incomplete(char *serialized)
{
    int ret;
    char *rpos;

    test_reset();

    while (buf_wsize(buf) < strlen(serialized)) {
        ck_assert_int_eq(dbuf_double(&buf), CC_OK);
    }

    buf_write(buf, serialized, strlen(serialized));

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    rpos = buf->rpos;
    ck_assert_int_eq(ret, PARSE_EUNFIN);
    ck_assert_ptr_eq(rpos, buf->rpos); // buffer did not advance
}

START_TEST(test_rsp_incomplete_leading_whitespace)
{
    test_rsp_incomplete(" ");
}
END_TEST

START_TEST(test_rsp_incomplete_type)
{
    test_rsp_incomplete("VALUE");
}
END_TEST

START_TEST(test_rsp_incomplete_data)
{
    test_rsp_incomplete("VALUE foo 123 3\r\nXY");
}
END_TEST

START_TEST(test_rsp_incomplete_number)
{
    test_rsp_incomplete("VALUE foo 123 3");
}
END_TEST

START_TEST(test_rsp_incomplete_data_crlf)
{
    test_rsp_incomplete("VALUE foo 123 3\r\nXYZ\r");
}
END_TEST

START_TEST(test_rsp_incomplete_key)
{
    test_rsp_incomplete("VALUE foo");
}
END_TEST

START_TEST(test_rsp_incomplete_flag)
{
    test_rsp_incomplete("VALUE foo 1");
}
END_TEST

START_TEST(test_rsp_incomplete_cas)
{
    test_rsp_incomplete("VALUE foo 1 2 1");
}
END_TEST

START_TEST(test_rsp_pool_basic)
{
#define POOL_SIZE 10
    int i;
    struct response *rsps[POOL_SIZE];
    response_options_st options = {.response_poolsize =
        {.type = OPTION_TYPE_UINT, .val.vuint = POOL_SIZE}};

    response_setup(&options, NULL);
    for (i = 0; i < POOL_SIZE; i++) {
        rsps[i] = response_borrow();
        ck_assert_msg(rsps[i] != NULL, "expected to borrow a response");
    }
    ck_assert_msg(response_borrow() == NULL, "expected response pool to be depleted");
    for (i = 0; i < POOL_SIZE; i++) {
        response_return(&rsps[i]);
        ck_assert_msg(rsps[i] == NULL, "expected response to be nulled after return");
    }
    response_teardown();
#undef POOL_SIZE
}
END_TEST

START_TEST(test_rsp_pool_chained)
{
#define POOL_SIZE 10
    int i;
    struct response *r, *nr, *rsps[POOL_SIZE];
    response_options_st options = {.response_poolsize =
        {.type = OPTION_TYPE_UINT, .val.vuint = POOL_SIZE}};

    response_setup(&options, NULL);

    r = response_borrow();
    ck_assert_msg(r != NULL, "expected to borrow a response");
    for (i = 1, nr = r; i < POOL_SIZE; ++i) {
        STAILQ_NEXT(nr, next) = response_borrow();
        nr = STAILQ_NEXT(nr, next);
        ck_assert_msg(nr != NULL, "expected to borrow response %d", i);
    }
    ck_assert_msg(response_borrow() == NULL, "expected response pool to be depleted");
    response_return_all(&r);
    ck_assert_msg(r == NULL, "expected response to be nulled after return");
    for (i = 0; i < POOL_SIZE; i++) {
        rsps[i] = response_borrow();
        ck_assert_msg(rsps[i] != NULL, "expected to borrow a response");
    }
    ck_assert_msg(response_borrow() == NULL, "expected response pool to be depleted");
    for (i = 0; i < POOL_SIZE; i++) {
        response_return(&rsps[i]);
        ck_assert_msg(rsps[i] == NULL, "expected response to be nulled after return");
    }

    response_teardown();
#undef POOL_SIZE
}
END_TEST

START_TEST(test_rsp_pool_metrics)
{
#define POOL_SIZE 2
    struct response *rsps[POOL_SIZE];
    response_metrics_st metrics =
        (response_metrics_st) { RESPONSE_METRIC(METRIC_INIT) };
    response_options_st options =
        (response_options_st) { RESPONSE_OPTION(OPTION_INIT) };
    options.response_poolsize.val.vuint = POOL_SIZE;

    response_setup(&options, &metrics);

    ck_assert_int_eq(metrics.response_borrow.counter, 0);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 2);

    rsps[0] = response_borrow();
    ck_assert_msg(rsps[0] != NULL, "expected to borrow a response");
    ck_assert_int_eq(metrics.response_borrow.counter, 1);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 1);

    rsps[1] = response_borrow();
    ck_assert_msg(rsps[1] != NULL, "expected to borrow a response");
    ck_assert_int_eq(metrics.response_borrow.counter, 2);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 0);

    ck_assert_msg(response_borrow() == NULL, "expected response pool to be depleted");
    ck_assert_int_eq(metrics.response_borrow.counter, 2);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 0);

    response_return(&rsps[1]);
    ck_assert_int_eq(metrics.response_borrow.counter, 2);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 1);

    response_return(&rsps[0]);
    ck_assert_int_eq(metrics.response_borrow.counter, 2);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 2);

    rsps[0] = response_borrow();
    ck_assert_msg(rsps[0] != NULL, "expected to borrow a response");
    ck_assert_int_eq(metrics.response_borrow.counter, 3);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 1);

    response_return(&rsps[0]);
    ck_assert_int_eq(metrics.response_borrow.counter, 3);
    ck_assert_int_eq(metrics.response_create.counter, 2);
    ck_assert_int_eq(metrics.response_free.counter, 2);

    response_teardown();
#undef POOL_SIZE
}
END_TEST

START_TEST(test_req_pool_basic)
{
#define POOL_SIZE 10
    int i;
    struct request *reqs[POOL_SIZE];
    request_options_st options = {.request_poolsize =
        {.type = OPTION_TYPE_UINT, .val.vuint = POOL_SIZE}};

    request_setup(&options, NULL);

    for (i = 0; i < POOL_SIZE; i++) {
        reqs[i] = request_borrow();
        ck_assert_msg(reqs[i] != NULL, "expected to borrow a request");
    }
    ck_assert_msg(request_borrow() == NULL, "expected request pool to be depleted");
    for (i = 0; i < POOL_SIZE; i++) {
        request_return(&reqs[i]);
        ck_assert_msg(reqs[i] == NULL, "expected request to be nulled after return");
    }

    request_teardown();
#undef POOL_SIZE
}
END_TEST

/*
 * test suite
 */
static Suite *
memcache_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic requests */
    TCase *tc_basic_req = tcase_create("basic request");
    suite_add_tcase(s, tc_basic_req);

    tcase_add_test(tc_basic_req, test_quit);
    tcase_add_test(tc_basic_req, test_delete);
    tcase_add_test(tc_basic_req, test_delete_noreply);
    tcase_add_test(tc_basic_req, test_get);
    tcase_add_test(tc_basic_req, test_multikey);
    tcase_add_test(tc_basic_req, test_gets);
    tcase_add_test(tc_basic_req, test_set);
    tcase_add_test(tc_basic_req, test_add_noreply);
    tcase_add_test(tc_basic_req, test_replace_noreply);
    tcase_add_test(tc_basic_req, test_cas);
    tcase_add_test(tc_basic_req, test_append);
    tcase_add_test(tc_basic_req, test_prepend_noreply);
    tcase_add_test(tc_basic_req, test_incr);
    tcase_add_test(tc_basic_req, test_decr_noreply);
    tcase_add_test(tc_basic_req, test_partial_header);
    tcase_add_test(tc_basic_req, test_partial_value);

    /* basic responses */
    TCase *tc_basic_rsp = tcase_create("basic response");
    suite_add_tcase(s, tc_basic_rsp);

    tcase_add_test(tc_basic_rsp, test_ok);
    tcase_add_test(tc_basic_rsp, test_end);
    tcase_add_test(tc_basic_rsp, test_stored);
    tcase_add_test(tc_basic_rsp, test_exists);
    tcase_add_test(tc_basic_rsp, test_deleted);
    tcase_add_test(tc_basic_rsp, test_notfound);
    tcase_add_test(tc_basic_rsp, test_notstored);
    tcase_add_test(tc_basic_rsp, test_stat);
    tcase_add_test(tc_basic_rsp, test_value);
    tcase_add_test(tc_basic_rsp, test_numeric);
    tcase_add_test(tc_basic_rsp, test_servererror);
    tcase_add_test(tc_basic_rsp, test_clienterror);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_leading_whitespace);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_type);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_data);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_number);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_data_crlf);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_key);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_flag);
    tcase_add_test(tc_basic_rsp, test_rsp_incomplete_cas);

    TCase *tc_rsp_pool = tcase_create("response pool");
    suite_add_tcase(s, tc_rsp_pool);

    tcase_add_test(tc_rsp_pool, test_rsp_pool_basic);
    tcase_add_test(tc_rsp_pool, test_rsp_pool_chained);
    tcase_add_test(tc_rsp_pool, test_rsp_pool_metrics);

    TCase *tc_req_pool = tcase_create("request pool");
    suite_add_tcase(s, tc_req_pool);

    tcase_add_test(tc_req_pool, test_req_pool_basic);

    return s;
}

/* TODO(yao): move main to a different file, keep most test files main-less */
int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = memcache_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

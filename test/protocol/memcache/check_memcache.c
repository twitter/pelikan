#include <protocol/memcache_include.h>

#include <cc_array.h>
#include <cc_define.h>
#include <buffer/cc_buf.h>
#include <cc_bstring.h>

#include <check.h>

#include <stdlib.h>
#include <stdio.h>

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
    buf_setup(BUF_INIT_SIZE, NULL);
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
    buf_teardown();
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

START_TEST(test_stats)
{
#define SERIALIZED "stats\r\n"

    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    req->type = REQ_STATS;
    ret = compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    request_reset(req);
    ret = parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->rstate == REQ_PARSED);
    ck_assert(req->type == REQ_STATS);
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

START_TEST(test_stat)
{
#define SERIALIZED "STAT memory_curr 24642\r\n"
#define NAME "memory_curr"
#define METRIC 24642

    int ret;
    int len = sizeof(SERIALIZED) - 1;
    struct metric met = {.name = NAME, .type = METRIC_GAUGE, .gauge = METRIC};

    test_reset();

    /* compose */
    rsp->type = RSP_STAT;
    rsp->met = &met;
    ret = compose_rsp(&buf, rsp);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    response_reset(rsp);
    ret = parse_rsp(rsp, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(rsp->rstate == RSP_PARSED);
    ck_assert(rsp->type == RSP_STAT);
    ck_assert_int_eq(bstring_compare(&rsp->key, &str2bstr(NAME)), 0);
    ck_assert_int_eq(rsp->vint, METRIC);
    ck_assert(buf->rpos == buf->wpos);
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
    tcase_add_test(tc_basic_req, test_stats);
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

    /* basic requests */
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

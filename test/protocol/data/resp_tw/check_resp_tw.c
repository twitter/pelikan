#include <protocol/data/resp_tw_include.h>

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_log.h>

#include <check.h>

#include <stdio.h>
#include <stdlib.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "resp"
#define DEBUG_LOG SUITE_NAME ".log"

struct request *req;
struct response *rsp;
struct buf *buf;

/* A simple wrapper around START_TEST that emits a log message
 * when the test starts. Meant to ease debugging.
 */
#define LOGGED_TEST(test_name)                 \
    START_TEST(test_name)                      \
        log_info("starting test " #test_name); 

#define ck_assert_str_eq_len(X, Y, len)                                  \
    do {                                                                 \
        const char *a = X;                                               \
        const char *b = Y;                                               \
        int l = len;                                                     \
        ck_assert_msg(                                                   \
            cc_bcmp(a, b, l) == 0,                                       \
            "Assertion %s == %s failed. %s == \"%.*s\", %s == \"%.*s\"", \
            #X, #Y, #X, l, a, #Y, l, b                                   \
        );                                                               \
    } while (0)

/* utilities */
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

#define TEST_SERIALIZE

/**************
 * test cases *
 **************/

LOGGED_TEST(test_simple_string)
{
#define STR "foobar"
#define SERIALIZED "+" STR "\r\n"

    struct element el_c, el_p;
    int ret;
    int len = sizeof(SERIALIZED) - 1;
    char *pos;

    test_reset();

    /* compose */
    el_c.type = ELEM_STR;
    el_c.bstr = str2bstr(STR);
    ret = compose_element(&buf, &el_c);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    pos = buf->rpos + 1;
    ret = parse_element(&el_p, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el_p.type == ELEM_STR);
    ck_assert(el_p.bstr.len == sizeof(STR) - 1);
    ck_assert(el_p.bstr.data == pos);

#undef SERIALIZED
#undef STR
}
END_TEST

LOGGED_TEST(test_error)
{
#define ERR "something is wrong"
#define SERIALIZED "-" ERR "\r\n"

    struct element el_c, el_p;
    int ret;
    int len = sizeof(SERIALIZED) - 1;
    char *pos;

    test_reset();

    /* compose */
    el_c.type = ELEM_ERR;
    el_c.bstr = str2bstr(ERR);
    ret = compose_element(&buf, &el_c);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    pos = buf->rpos + 1;
    ret = parse_element(&el_p, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el_p.type == ELEM_ERR);
    ck_assert(el_p.bstr.len == sizeof(ERR) - 1);
    ck_assert(el_p.bstr.data == pos);

#undef SERIALIZED
#undef ERR
}
END_TEST

LOGGED_TEST(test_integer)
{
#define OVERSIZE ":19223372036854775807\r\n"
#define INVALID1 ":123lOl456\r\n"
#define INVALID2 ":\r\n"

    struct element el_c, el_p;
    int ret;

    struct int_pair {
        char *serialized;
        uint64_t num;
    } pairs[3] = {
        {":-1\r\n", -1},
        {":9223372036854775807\r\n", 9223372036854775807},
        {":128\r\n", 128}
    };


    test_reset();
    for (int i = 0; i < 3; i++) {
        size_t len = strlen(pairs[i].serialized);

        buf_reset(buf);
        el_c.type = ELEM_NUMBER;
        el_c.num = pairs[i].num;
        ret = compose_element(&buf, &el_c);
        ck_assert(ret == len);
        ck_assert_int_eq(cc_bcmp(buf->rpos, pairs[i].serialized, len), 0);

        el_p.type = ELEM_UNKNOWN;
        ret = parse_element(&el_p, buf);
        ck_assert_int_eq(ret, PARSE_OK);
        ck_assert(buf->rpos == buf->wpos);
        ck_assert(el_p.type == ELEM_NUMBER);
        ck_assert(el_p.num == pairs[i].num);
    }

    buf_reset(buf);
    buf_write(buf, OVERSIZE, sizeof(OVERSIZE) - 1);
    ret = parse_element(&el_p, buf);
    ck_assert_int_eq(ret, PARSE_EOVERSIZE);

    buf_reset(buf);
    buf_write(buf, INVALID1, sizeof(INVALID1) - 1);
    ret = parse_element(&el_p, buf);
    ck_assert_int_eq(ret, PARSE_EINVALID);

    buf_reset(buf);
    buf_write(buf, INVALID2, sizeof(INVALID2) - 1);
    ret = parse_element(&el_p, buf);
    ck_assert_int_eq(ret, PARSE_EINVALID);

#undef INVALID2
#undef INVALID1
#undef OVERSIZE
}
END_TEST

LOGGED_TEST(test_bulk_string)
{
#define BULK "foo bar\r\n"
#define SERIALIZED "$9\r\n" BULK "\r\n"
#define EMPTY "$0\r\n\r\n"

    struct element el_c, el_p;
    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    el_c.type = ELEM_BLOB_STR;
    el_c.bstr = str2bstr(BULK);
    ret = compose_element(&buf, &el_c);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d, out: %s", len, ret, buf->begin);
    ck_assert_msg(
        cc_bcmp(buf->rpos, SERIALIZED, ret) == 0, 
        "string comparison failed: '%s' != '%s'", buf->rpos, 
        SERIALIZED
    );
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    ck_assert_int_eq(parse_element(&el_p, buf), PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el_p.type == ELEM_BLOB_STR);
    ck_assert(el_p.bstr.len == sizeof(BULK) - 1);
    ck_assert(el_p.bstr.data + el_p.bstr.len == buf->rpos - CRLF_LEN);
    ck_assert(buf->rpos == buf->wpos);

    /* empty string */
    buf_reset(buf);
    len = sizeof(EMPTY) - 1;
    el_c.bstr = null_bstring;
    ret = compose_element(&buf, &el_c);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d, out: %s", len, ret, buf->begin);
    ck_assert_int_eq(cc_bcmp(buf->rpos, EMPTY, ret), 0);
    ck_assert_int_eq(parse_element(&el_p, buf), PARSE_OK);
    ck_assert(el_p.bstr.len == 0);


#undef EMPTY
#undef SERIALIZED
#undef BULK
}
END_TEST

LOGGED_TEST(test_array)
{
#define SERIALIZED "*2\r\n+foo\r\n$4\r\nbarr\r\n"
#define NELEM 2

    size_t len = sizeof(SERIALIZED) - 1;
    uint64_t nelem;

    test_reset();

    buf_write(buf, SERIALIZED, len);
    ck_assert(token_is_array(buf));
    ck_assert_int_eq(token_array_nelem(&nelem, buf), PARSE_OK);
    ck_assert_int_eq(nelem, NELEM);

#undef NELEM
#undef SERIALIZED
}
END_TEST

LOGGED_TEST(test_nil_blob_str_invalid)
{
#define NIL_BULK "$-1\r\n"

    size_t len = sizeof(NIL_BULK) - 1;
    struct element el;

    test_reset();

    buf_write(buf, NIL_BULK, len);
    el.type = ELEM_UNKNOWN;
    ck_assert_int_eq(parse_element(&el, buf), PARSE_EINVALID);

#undef NIL_BULK
}
END_TEST

LOGGED_TEST(test_unfin_token)
{
    char *token[13] = {
        "+hello ",
        "-err",
        "-err\r",
        ":5",
        ":5\r",
        "$5",
        "$5\r",
        "$5\r\n",
        "$5\r\nabc",
        "$5\r\nabcde\r",
        "*5",
        "*5\r",
    };
    char *pos;
    size_t len;

    for (int i = 0; i < 10; i++) {
        struct element el;

        len = strlen(token[i]);
        buf_reset(buf);
        buf_write(buf, token[i], len);
        pos = buf->rpos;
        ck_assert_int_eq(parse_element(&el, buf), PARSE_EUNFIN);
        ck_assert(buf->rpos == pos);
    }

    for (int i = 10; i < 12; i++) {
        uint64_t nelem;

        len = strlen(token[i]);
        buf_reset(buf);
        buf_write(buf, token[i], len);
        pos = buf->rpos;
        ck_assert_int_eq(token_array_nelem(&nelem, buf), PARSE_EUNFIN);
        ck_assert(buf->rpos == pos);
    }
}
END_TEST

LOGGED_TEST(test_double_unsupported)
{
#define DOUBLE ",3.14152695\r\n"
    size_t len = sizeof(DOUBLE) - 1;

    test_reset();
    buf_write(buf, DOUBLE, len);

    struct element el;
    ck_assert_int_eq(parse_element(&el, buf), PARSE_ENOTSUPPORTED);
#undef DOUBLE
}
END_TEST

/*
 * request
 */

LOGGED_TEST(test_quit)
{
#define QUIT "quit"
#define SERIALIZED "*1\r\n$4\r\n" QUIT "\r\n"
#define INVALID "*2\r\n$4\r\n" QUIT "\r\n$3\r\nnow\r\n"
    int ret;
    struct element *el;

    test_reset();

    req->type = REQ_QUIT;
    el = array_push(req->token);
    el->type = ELEM_BLOB_STR;
    el->bstr = (struct bstring){sizeof(QUIT) - 1, QUIT};
    ret = compose_req(&buf, req);
    ck_assert_int_eq(ret, sizeof(SERIALIZED) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    el->type = ELEM_UNKNOWN; /* this effectively resets *el */
    request_reset(req);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);
    ck_assert_int_eq(req->type, REQ_QUIT);
    ck_assert_int_eq(req->token->nelem, 1);
    el = array_first(req->token);
    ck_assert_int_eq(el->type, ELEM_BLOB_STR);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, QUIT, sizeof(QUIT) - 1), 0);

    /* invalid number of arguments */
    test_reset();
    buf_write(buf, INVALID, sizeof(INVALID) - 1);
    ck_assert_int_eq(parse_req(req, buf), PARSE_EINVALID);
#undef INVALID
#undef SERIALIZED
#undef QUIT
}
END_TEST


LOGGED_TEST(test_ping)
{
#define PING "ping"
#define VAL "hello"
#define S_PING "*1\r\n$4\r\n" PING "\r\n"
#define S_ECHO "*2\r\n$4\r\n" PING "\r\n$5\r\nhello\r\n"
    int ret;
    struct element *el;

    test_reset();

    /* simple ping */
    buf_write(buf, S_PING, sizeof(S_PING) - 1);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);
    ck_assert_int_eq(req->type, REQ_PING);

    /* ping as echo */
    test_reset();

    req->type = REQ_PING;
    el = array_push(req->token);
    el->type = ELEM_BLOB_STR;
    el->bstr = (struct bstring){sizeof(PING) - 1, PING};
    el = array_push(req->token);
    el->type = ELEM_BLOB_STR;
    el->bstr = (struct bstring){sizeof(VAL) - 1, VAL};
    ret = compose_req(&buf, req);
    ck_assert_int_eq(ret, sizeof(S_ECHO) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, S_ECHO, ret), 0);

    el->type = ELEM_UNKNOWN; /* resets *el */
    request_reset(req);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);
    ck_assert_int_eq(req->type, REQ_PING);
    ck_assert_int_eq(req->token->nelem, 2);
    el = array_first(req->token);
    ck_assert_int_eq(el->type, ELEM_BLOB_STR);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, PING, sizeof(PING) - 1), 0);
    el = array_get(req->token, 1);
    ck_assert_int_eq(el->type, ELEM_BLOB_STR);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, VAL, sizeof(VAL) - 1), 0);
#undef S_ECHO
#undef ECHO
#undef S_PING
#undef QUIT
}
END_TEST

LOGGED_TEST(test_unfin_req)
{
    char *token[4] = {
        "*2\r\n",
        "*2\r\n$3\r\n",
        "*2\r\n$3\r\nfoo\r\n",
        "*2\r\n$3\r\nfoo\r\n$3\r\n",
    };

    for (int i = 0; i < 4; i++) {
        char *pos;
        size_t len;

        len = strlen(token[i]);
        buf_reset(buf);
        buf_write(buf, token[i], len);
        pos = buf->rpos;
        ck_assert_int_eq(parse_req(req, buf), PARSE_EUNFIN);
        ck_assert(buf->rpos == pos);
    }
}
END_TEST

/*
 * response
 */
LOGGED_TEST(test_ok)
{
#define OK "OK"
#define SERIALIZED "+" OK "\r\n"
    int ret;
    struct element *el;

    test_reset();

    rsp->type = ELEM_STR;
    el = array_push(rsp->token);
    el->type = ELEM_STR;
    el->bstr = (struct bstring){sizeof(OK) - 1, OK};
    ret = compose_rsp(&buf, rsp);
    ck_assert_int_eq(ret, sizeof(SERIALIZED) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    el->type = ELEM_UNKNOWN; /* resets *el */
    response_reset(rsp);
    ck_assert_int_eq(parse_rsp(rsp, buf), PARSE_OK);
    ck_assert_int_eq(rsp->type, ELEM_STR);
    ck_assert_int_eq(rsp->token->nelem, 1);
    el = array_first(rsp->token);
    ck_assert_int_eq(el->type, ELEM_STR);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, OK, sizeof(OK) - 1), 0);
#undef SERIALIZED
#undef OK
}
END_TEST

LOGGED_TEST(test_array_reply)
{
#define SERIALIZED "*5\r\n:-10\r\n_\r\n-ERR invalid arg\r\n+foo\r\n$5\r\nHELLO\r\n"
    size_t len = sizeof(SERIALIZED) - 1;
    struct element *el;

    test_reset();

    buf_write(buf, SERIALIZED, len);
    ck_assert_int_eq(parse_rsp(rsp, buf), PARSE_OK);
    ck_assert_int_eq(rsp->type, ELEM_ARRAY);
    ck_assert_int_eq(rsp->token->nelem, 5);
    el = array_first(rsp->token);
    ck_assert_int_eq(el->type, ELEM_NUMBER);
    el = array_get(rsp->token, 1);
    ck_assert_int_eq(el->type, ELEM_NIL);
    el = array_get(rsp->token, 2);
    ck_assert_int_eq(el->type, ELEM_ERR);
    el = array_get(rsp->token, 3);
    ck_assert_int_eq(el->type, ELEM_STR);
    el = array_get(rsp->token, 4);
    ck_assert_int_eq(el->type, ELEM_BLOB_STR);
    ck_assert_int_eq(el->bstr.len, 5);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, "HELLO", 5), 0);
    ck_assert_int_eq(buf_rsize(buf), 0);
    ck_assert_int_eq(rsp->attrs->nelem, 0);

    ck_assert_int_eq(compose_rsp(&buf, rsp), len);
    ck_assert_int_eq(buf_rsize(buf), len);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, len), 0);
#undef SERIALIZED
}
END_TEST

LOGGED_TEST(test_reply_with_attributes)
{
#define SERIALIZED "|1\r\n+sTTL\r\n:15\r\n_\r\n"
    size_t len = sizeof(SERIALIZED) - 1;
    struct attribute_entry *entry;

    test_reset();

    buf_write(buf, SERIALIZED, len);
    ck_assert_int_eq(parse_rsp(rsp, buf), PARSE_OK);
    ck_assert_int_eq(rsp->type, ELEM_NIL);
    ck_assert_int_eq(rsp->attrs->nelem, 1);
    entry = array_first(rsp->attrs);
    ck_assert_int_eq(entry->key.type, ELEM_STR);
    ck_assert_str_eq_len(entry->key.bstr.data, "sTTL", 4);
    ck_assert_int_eq(entry->val.type, ELEM_NUMBER);
    ck_assert_int_eq(entry->val.num, 15);

    ck_assert_int_eq(compose_rsp(&buf, rsp), len);
    ck_assert_int_eq(buf_rsize(buf), len);
    ck_assert_str_eq_len(buf->rpos, SERIALIZED, len);
#undef SERIALIZED
}
END_TEST

LOGGED_TEST(test_map_reply)
{
#define TEST "test"
#define OTHER "other"
#define SERIALIZED "%2\r\n+" TEST "\r\n:3\r\n+" OTHER "\r\n:4\r\n"
    size_t test_len = sizeof(TEST) - 1;
    size_t other_len = sizeof(OTHER) - 1;
    size_t len = sizeof(SERIALIZED) - 1;
    struct element *el;

    test_reset();

    buf_write(buf, SERIALIZED, len);
    ck_assert_int_eq(parse_rsp(rsp, buf), PARSE_OK);
    ck_assert_int_eq(rsp->attrs->nelem, 0);
    ck_assert_int_eq(rsp->token->nelem, 4);
    el = array_get(rsp->token, 0);
    ck_assert_int_eq(el->type, ELEM_STR);
    ck_assert_str_eq_len(el->bstr.data, TEST, test_len);
    el = array_get(rsp->token, 1);
    ck_assert_int_eq(el->type, ELEM_NUMBER);
    ck_assert_int_eq(el->num, 3);
    el = array_get(rsp->token, 2);
    ck_assert_int_eq(el->type, ELEM_STR);
    ck_assert_str_eq_len(el->bstr.data, OTHER, other_len);
    el = array_get(rsp->token, 3);
    ck_assert_int_eq(el->type, ELEM_NUMBER);
    ck_assert_int_eq(el->num, 4);

    ck_assert_int_eq(compose_rsp(&buf, rsp), len);
    ck_assert_int_eq(buf_rsize(buf), len);
    ck_assert_str_eq_len(buf->rpos, SERIALIZED, len);
#undef TEST
#undef OTHER
#undef SERIALIZED
}
END_TEST

/*
 * edge cases
 */
LOGGED_TEST(test_empty_buf)
{
    struct element el;
    test_reset();

    ck_assert(!token_is_array(buf));
    ck_assert_int_eq(parse_element(&el, buf), PARSE_EUNFIN);
    ck_assert_int_eq(parse_rsp(rsp, buf), PARSE_EUNFIN);
    ck_assert_int_eq(parse_req(req, buf), PARSE_EUNFIN);
}
END_TEST

LOGGED_TEST(test_large_bulk_string)
{
    /*
     * Test a bulk string with a size just above the maximum
     * allowed size (512 MB - 1). If bulk string handling is
     * implemented correctly then this should return 
     * PARSE_EUNFIN.
     */

#define SERIALIZED "$536870911\r\n\r\n"
    struct element el;

    test_reset();

    buf_write(buf, SERIALIZED, sizeof(SERIALIZED) - 1);

    ck_assert_int_eq(parse_element(&el, buf), PARSE_EUNFIN);
#undef SERIALIZED
}
END_TEST

/*
 * request/response pool
 */

LOGGED_TEST(test_req_pool_basic)
{
#define POOL_SIZE 10
    int i;
    struct request *reqs[POOL_SIZE];
    request_options_st options = {
        .request_ntoken = {.type = OPTION_TYPE_UINT, .val.vuint = REQ_NTOKEN},
        .request_poolsize = {.type = OPTION_TYPE_UINT, .val.vuint = POOL_SIZE}};

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

LOGGED_TEST(test_rsp_pool_basic)
{
#define POOL_SIZE 10
    int i;
    struct response *rsps[POOL_SIZE];
    response_options_st options = {
        .response_ntoken = {.type = OPTION_TYPE_UINT, .val.vuint = RSP_NTOKEN},
        .response_poolsize = {.type = OPTION_TYPE_UINT, .val.vuint = POOL_SIZE}};

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


/*
 * Test Suite
 */
static Suite *
resp_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* token */
    TCase *tc_token = tcase_create("token");
    suite_add_tcase(s, tc_token);

    tcase_add_test(tc_token, test_simple_string);
    tcase_add_test(tc_token, test_error);
    tcase_add_test(tc_token, test_integer);
    tcase_add_test(tc_token, test_bulk_string);
    tcase_add_test(tc_token, test_array);
    tcase_add_test(tc_token, test_nil_blob_str_invalid);
    tcase_add_test(tc_token, test_unfin_token);
    tcase_add_test(tc_token, test_double_unsupported);

    /* basic requests */
    TCase *tc_request = tcase_create("request");
    suite_add_tcase(s, tc_request);

    tcase_add_test(tc_request, test_quit);
    tcase_add_test(tc_request, test_ping);
    tcase_add_test(tc_request, test_unfin_req);

    /* basic responses */
    TCase *tc_response = tcase_create("response");
    suite_add_tcase(s, tc_response);

    tcase_add_test(tc_response, test_ok);
    tcase_add_test(tc_response, test_map_reply);
    tcase_add_test(tc_response, test_array_reply);
    tcase_add_test(tc_response, test_reply_with_attributes);

    /* edge cases */
    TCase *tc_edge = tcase_create("edge cases");
    suite_add_tcase(s, tc_edge);
    tcase_add_test(tc_edge, test_empty_buf);
    tcase_add_test(tc_edge, test_large_bulk_string);
    tcase_add_test(tc_edge, test_large_bulk_string);

    /* req/rsp objects, pooling */
    TCase *tc_pool = tcase_create("request/response pool");
    suite_add_tcase(s, tc_pool);

    tcase_add_test(tc_pool, test_req_pool_basic);
    tcase_add_test(tc_pool, test_rsp_pool_basic);

    return s;
}

int
main(void)
{
    int nfail;

    debug_options_st opts = {
        DEBUG_OPTION(OPTION_INIT)
    };
    opts.debug_log_level.val.vuint = LOG_INFO;
    debug_setup(&opts);
    test_setup();

    Suite *suite = resp_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV);
    
    nfail = srunner_ntests_failed(srunner);

    srunner_free(srunner);

    test_teardown();
    debug_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

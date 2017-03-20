#include <protocol/data/redis_include.h>

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_define.h>

#include <check.h>

#include <stdio.h>
#include <stdlib.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "redis"
#define DEBUG_LOG  SUITE_NAME ".log"

struct request *req;
struct buf *buf;

/*
 * utilities
 */
static void
test_setup(void)
{
    req = request_create();
    buf = buf_create();
}

static void
test_reset(void)
{
    request_reset(req);
    buf_reset(buf);
}

static void
test_teardown(void)
{
    buf_destroy(&buf);
    request_destroy(&req);
}

/**************
 * test cases *
 **************/

/*
 * token
 */
START_TEST(test_simple_string)
{
#define SERIALIZED "+foobar\r\n"
#define STR "foobar"

    struct element el;
    int ret;
    int len = sizeof(SERIALIZED) - 1;
    char *pos;

    test_reset();

    /* compose */
    el.type = ELEM_STR;
    el.bstr = str2bstr(STR);
    ret = compose_element(&buf, &el);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    pos = buf->rpos + 1;
    ret = parse_element(&el, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el.type == ELEM_STR);
    ck_assert(el.bstr.len == sizeof(STR) - 1);
    ck_assert(el.bstr.data == pos);

#undef STR
#undef SERIALIZED
}
END_TEST

START_TEST(test_error)
{
#define SERIALIZED "-something is wrong\r\n"
#define ERR "something is wrong"

    struct element el;
    int ret;
    int len = sizeof(SERIALIZED) - 1;
    char *pos;

    test_reset();

    /* compose */
    el.type = ELEM_ERR;
    el.bstr = str2bstr(ERR);
    ret = compose_element(&buf, &el);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    pos = buf->rpos + 1;
    ret = parse_element(&el, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el.type == ELEM_ERR);
    ck_assert(el.bstr.len == sizeof(ERR) - 1);
    ck_assert(el.bstr.data == pos);

#undef ERR
#undef SERIALIZED
}
END_TEST

START_TEST(test_integer)
{
#define SERIALIZED_1 ":-1\r\n"
#define SERIALIZED_2 ":9223372036854775807\r\n"
#define SERIALIZED_3 ":128\r\n"
#define INT_1 -1
#define INT_2 9223372036854775807LL
#define INT_3 128

    struct element el;
    int ret;

    test_reset();
    el.type = ELEM_INT;

    el.num = INT_1;
    ret = compose_element(&buf, &el);
    ck_assert(ret == sizeof(SERIALIZED_1) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED_1, ret), 0);
    ret = parse_element(&el, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el.type == ELEM_INT);
    ck_assert(el.num == INT_1);

    el.num = INT_2;
    ret = compose_element(&buf, &el);
    ck_assert(ret == sizeof(SERIALIZED_2) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED_2, ret), 0);
    ret = parse_element(&el, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(el.type == ELEM_INT);
    ck_assert(el.num == INT_2);

    el.num = INT_3;
    ret = compose_element(&buf, &el);
    ck_assert(ret == sizeof(SERIALIZED_3) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED_3, ret), 0);
    ret = parse_element(&el, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(el.num == INT_3);

#undef INT_3
#undef INT_2
#undef INT_1
#undef SERIALIZED_3
#undef SERIALIZED_2
#undef SERIALIZED_1
}
END_TEST

START_TEST(test_bulk_string)
{
#define SERIALIZED "$9\r\nfoo bar\r\n\r\n"
#define BULK "foo bar\r\n"

    struct element el;
    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    el.type = ELEM_BULK;
    el.bstr = str2bstr(BULK);
    ret = compose_element(&buf, &el);
    ck_assert_msg(ret == len, "bytes expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    ret = parse_element(&el, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(buf->rpos == buf->wpos);
    ck_assert(el.type == ELEM_BULK);
    ck_assert(el.bstr.len == sizeof(BULK) - 1);
    ck_assert(el.bstr.data + el.bstr.len == buf->rpos - CRLF_LEN);

#undef BULK
#undef SERIALIZED
}
END_TEST

START_TEST(test_array)
{
#define SERIALIZED "*2\r\n+foo\r\n$4\r\nbarr\r\n"
#define NELEM 2
#define NULLARRAY "*-1\r\n"

    int len = sizeof(SERIALIZED) - 1;
    int nelem;

    test_reset();

    buf_write(buf, SERIALIZED, len);
    ck_assert(token_is_array(buf));
    ck_assert_int_eq(token_array_nelem(&nelem, buf), PARSE_OK);
    ck_assert_int_eq(nelem, NELEM);

#undef NULLARRAY
#undef NELEM
#undef SERIALIZED
}
END_TEST


/*
 * request
 */

START_TEST(test_quit)
{
#define QUIT "quit"
#define SERIALIZED "*1\r\n$4\r\nquit\r\n"
#define INVALID "*2\r\n$4\r\nquit\r\n$3\r\nnow\r\n"
    int ret;
    struct element *el;

    test_reset();

    req->type = REQ_QUIT;
    el = array_push(req->token);
    el->type = ELEM_BULK;
    el->bstr = (struct bstring){sizeof(QUIT) - 1, QUIT};
    ret = compose_req(&buf, req);
    ck_assert_int_eq(ret, sizeof(SERIALIZED) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    el->type = ELEM_UNKNOWN;
    request_reset(req);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);
    ck_assert_int_eq(req->type, REQ_QUIT);
    ck_assert_int_eq(req->token->nelem, 1);
    el = array_first(req->token);
    ck_assert_int_eq(el->type, ELEM_BULK);
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


START_TEST(test_ping)
{
#define PING "ping"
#define VAL "hello"
#define S_PING "*1\r\n$4\r\nping\r\n"
#define S_ECHO "*2\r\n$4\r\nping\r\n$5\r\nhello\r\n"
#define S_ECHO2 "*3\r\n$4\r\nping\r\n$5\r\nhello\r\n$5\r\nworld\r\n"
    int ret;
    struct element *el;

    test_reset();

    /* simple ping */
    buf_write(buf, S_PING, sizeof(S_PING) - 1);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);

    /* ping as echo */
    test_reset();

    req->type = REQ_PING;
    el = array_push(req->token);
    el->type = ELEM_BULK;
    el->bstr = (struct bstring){sizeof(PING) - 1, PING};
    el = array_push(req->token);
    el->type = ELEM_BULK;
    el->bstr = (struct bstring){sizeof(VAL) - 1, VAL};
    ret = compose_req(&buf, req);
    ck_assert_int_eq(ret, sizeof(S_ECHO) - 1);
    ck_assert_int_eq(cc_bcmp(buf->rpos, S_ECHO, ret), 0);

    el->type = ELEM_UNKNOWN;
    request_reset(req);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);
    ck_assert_int_eq(req->type, REQ_PING);
    ck_assert_int_eq(req->token->nelem, 2);
    el = array_first(req->token);
    ck_assert_int_eq(el->type, ELEM_BULK);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, PING, sizeof(PING) - 1), 0);
    el = array_get(req->token, 1);
    ck_assert_int_eq(el->type, ELEM_BULK);
    ck_assert_int_eq(cc_bcmp(el->bstr.data, VAL, sizeof(VAL) - 1), 0);

    /* more arguments */
    test_reset();
    buf_write(buf, S_ECHO2, sizeof(S_ECHO2) - 1);
    ck_assert_int_eq(parse_req(req, buf), PARSE_OK);
    ck_assert_int_eq(req->token->nelem, 3);
#undef S_ECHO2
#undef S_ECHO
#undef ECHO
#undef S_PING
#undef QUIT
}
END_TEST

/*
 * request/response pool
 */

START_TEST(test_req_pool_basic)
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

START_TEST(test_rsp_pool_basic)
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
 * test suite
 */
static Suite *
redis_suite(void)
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

    /* basic requests */
    TCase *tc_request = tcase_create("request");
    suite_add_tcase(s, tc_request);

    tcase_add_test(tc_request, test_quit);
    tcase_add_test(tc_request, test_ping);

    /* basic responses */

    /* req/rsp objects, pooling */
    TCase *tc_pool = tcase_create("request/response pool");
    suite_add_tcase(s, tc_pool);

    tcase_add_test(tc_pool, test_req_pool_basic);
    tcase_add_test(tc_pool, test_rsp_pool_basic);

    return s;
}

/* TODO(yao): move main to a different file, keep most test files main-less */
int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = redis_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

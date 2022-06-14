#include <protocol/admin/admin_include.h>

#include <buffer/cc_buf.h>

#include <check.h>

#define SUITE_NAME "admin"
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
    req = admin_request_create();
    rsp = admin_response_create();
    buf = buf_create();
}

static void
test_reset(void)
{
    admin_request_reset(req);
    admin_response_reset(rsp);
    buf_reset(buf);
}

static void
test_teardown(void)
{
    buf_destroy(&buf);
    admin_response_destroy(&rsp);
    admin_request_destroy(&req);
}

/**************
 * test cases *
 **************/

/*
 * basic admin requests
 */
START_TEST(test_quit)
{
#define SERIALIZED "quit\r\n"
    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    req->type = REQ_QUIT;
    ret = admin_compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    admin_request_reset(req);
    ret = admin_parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->state == REQ_PARSED);
    ck_assert(req->type = REQ_QUIT);
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
    ret = admin_compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    admin_request_reset(req);
    ret = admin_parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->state == REQ_PARSED);
    ck_assert(req->type = REQ_STATS);
#undef SERIALIZED
}
END_TEST

START_TEST(test_version)
{
#define SERIALIZED "version\r\n"
    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    req->type = REQ_VERSION;
    ret = admin_compose_req(&buf, req);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    admin_request_reset(req);
    ret = admin_parse_req(req, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(req->state == REQ_PARSED);
    ck_assert(req->type = REQ_VERSION);
#undef SERIALIZED
}
END_TEST

/*
 * test suite
 */
static Suite *
admin_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic admin requests */
    TCase *tc_basic_req = tcase_create("basic req");
    suite_add_tcase(s, tc_basic_req);

    tcase_add_test(tc_basic_req, test_quit);
    tcase_add_test(tc_basic_req, test_stats);
    tcase_add_test(tc_basic_req, test_version);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = admin_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VERBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

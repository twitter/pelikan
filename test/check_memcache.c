#include <stdlib.h>

#include <check.h>

#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_string.h>

#include <memcache/bb_request.h>

START_TEST(test_request_init)
{
    rstatus_t ret;
    struct request req;

    ret = request_init(&req);
    ck_assert_msg(ret == CC_OK, "request init failed");

}
END_TEST

START_TEST(test_request_parse_hdr)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request req;
    rstatus_t ret;

    cmd = "quit\r\n";
    request_init(&req);
    buf = mbuf_get();
    mbuf_copy(buf, cmd, cc_strlen(cmd));
    ret = request_parse_hdr(&req, buf);
    ck_assert(ret == CC_OK);
    ck_assert_msg(req.verb == QUIT);

    request_init(&req);
    buf = mbuf_get();
    cmd = "quit\r\n";
    mbuf_copy(buf, cmd, cc_strlen(cmd));
    ret = request_parse_hdr(&req, buf);
    ck_assert(ret == CC_OK);
    ck_assert_msg(req.verb == QUIT);
}
END_TEST

Suite *
memcache_suite(void)
{
    Suite *s = suite_create("memcache");

    /* basic tests */
    TCase *tc_basic = tcase_create("basic");
    tcase_add_test(tc_basic, test_request_init);
    tcase_add_test(tc_basic, test_request_parse_hdr);
    suite_add_tcase(s, tc_basic);

    return s;
}

int main(void)
{
    int nfail;
    struct mbuf *buf;

    /* setup */
    mbuf_setup(MBUF_SIZE);

    Suite *suite = memcache_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_run_all(srunner, CK_NORMAL);
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    mbuf_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

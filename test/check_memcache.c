#include <stdlib.h>

#include <check.h>

#include <cc_array.h>
#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_string.h>

#include <memcache/bb_request.h>


START_TEST(test_quit)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    rstatus_t status;

    cmd = (uint8_t *)"quit\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert(status == CC_OK);
    ck_assert(req->verb == QUIT);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_delete)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"delete foo\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert_msg(req->verb == DELETE);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

Suite *
memcache_suite(void)
{
    Suite *s = suite_create("memcache");

    /* basic tests */
    TCase *tc_basic = tcase_create("basic");
    tcase_add_test(tc_basic, test_quit);
    tcase_add_test(tc_basic, test_delete);
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

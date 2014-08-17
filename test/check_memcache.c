#include <stdlib.h>

#include <check.h>

#include <cc_array.h>
#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_string.h>

#include <memcache/bb_request.h>

/* TODO(yao): simplify buf & req setup/teardown */

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

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
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
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == DELETE);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = array_get_idx(keys, 0);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_get)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"get foo\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == GET);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = array_get_idx(keys, 0);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_get_multi)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"get foo bar\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == GET);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 2);
    key = array_get_idx(keys, 0);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    key = array_get_idx(keys, 1);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "bar", 3) == 0);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_gets)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"gets foo\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == GETS);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_gets_multi)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"gets foo bar\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == GETS);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 2);
    key = array_get_idx(keys, 0);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    key = array_get_idx(keys, 1);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "bar", 3) == 0);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_set)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"set foo 111 86400 3\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == SET);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    ck_assert(req->flag == 111);
    ck_assert(req->expiry == 86400);
    ck_assert(req->vlen == 3);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_add)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"add foO 112 86401 4\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == ADD);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foO", 3) == 0);
    ck_assert(req->flag == 112);
    ck_assert(req->expiry == 86401);
    ck_assert(req->vlen == 4);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_replace)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"replace fOO 113 86402 5\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == REPLACE);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "fOO", 3) == 0);
    ck_assert(req->flag == 113);
    ck_assert(req->expiry == 86402);
    ck_assert(req->vlen == 5);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_cas)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"cas foo 111 86400 3 22\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == CAS);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    ck_assert(req->flag == 111);
    ck_assert(req->expiry == 86400);
    ck_assert(req->vlen == 3);
    ck_assert(req->cas == 22);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_append)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"append foo 0 0 3\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == APPEND);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    ck_assert(req->flag == 0);
    ck_assert(req->expiry == 0);
    ck_assert(req->vlen == 3);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_prepend)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"prepend foo 0 0 5\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == PREPEND);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    ck_assert(req->flag == 0);
    ck_assert(req->expiry == 0);
    ck_assert(req->vlen == 5);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_incr)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"incr num 31\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == INCR);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "num", 3) == 0);
    ck_assert(req->delta == 31);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_decr)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"decr num 28\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == DECR);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = keys->data;
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "num", 3) == 0);
    ck_assert(req->delta == 28);

    request_destroy(req);
    mbuf_put(buf);
}
END_TEST

START_TEST(test_delete_noreply)
{
    uint8_t *cmd;
    struct mbuf *buf;
    struct request *req;
    struct array *keys;
    struct bstring *key;
    rstatus_t status;

    cmd = (uint8_t *)"delete foo noreply\r\n";
    req = request_create();
    buf = mbuf_get();
    mbuf_copy(buf, cmd, (uint32_t)cc_strlen(cmd));
    status = request_parse_hdr(req, buf);

    ck_assert_msg(status == CC_OK, "status: %d", (int)status);
    ck_assert(req->pstate == PARSED);
    ck_assert(req->verb == DELETE);
    keys = req->keys;
    ck_assert(array_nelem(keys) == 1);
    key = array_get_idx(keys, 0);
    ck_assert(key->len == 3);
    ck_assert(cc_strncmp(key->data, "foo", 3) == 0);
    ck_assert(req->noreply == 1);

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
    tcase_add_test(tc_basic, test_get);
    tcase_add_test(tc_basic, test_get_multi);
    tcase_add_test(tc_basic, test_gets);
    tcase_add_test(tc_basic, test_gets_multi);
    tcase_add_test(tc_basic, test_set);
    tcase_add_test(tc_basic, test_add);
    tcase_add_test(tc_basic, test_replace);
    tcase_add_test(tc_basic, test_cas);
    tcase_add_test(tc_basic, test_append);
    tcase_add_test(tc_basic, test_prepend);
    tcase_add_test(tc_basic, test_incr);
    tcase_add_test(tc_basic, test_decr);
    tcase_add_test(tc_basic, test_delete_noreply);
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

#include <protocol/admin_include.h>

#include <buffer/cc_buf.h>

#include <check.h>

#define SUITE_NAME "admin"
#define DEBUG_LOG  SUITE_NAME ".log"

struct op *op;
struct reply *rep;
struct buf *buf;

/*
 * utilities
 */
static void
test_setup(void)
{
    buf_setup(BUF_INIT_SIZE, NULL);
    op = op_create();
    rep = reply_create();
    buf = buf_create();
}

static void
test_reset(void)
{
    op_reset(op);
    reply_reset(rep);
    buf_reset(buf);
}

static void
test_teardown(void)
{
    buf_destroy(&buf);
    reply_destroy(&rep);
    op_destroy(&op);
    buf_teardown();
}

/**************
 * test cases *
 **************/

/*
 * basic ops
 */
START_TEST(test_quit)
{
#define SERIALIZED "quit\r\n"
    int ret;
    int len = sizeof(SERIALIZED) - 1;

    test_reset();

    /* compose */
    op->type = OP_QUIT;
    ret = compose_op(&buf, op);
    ck_assert_msg(ret == len, "expected: %d, returned: %d", len, ret);
    ck_assert_int_eq(cc_bcmp(buf->rpos, SERIALIZED, ret), 0);

    /* parse */
    op_reset(op);
    ret = parse_op(op, buf);
    ck_assert_int_eq(ret, PARSE_OK);
    ck_assert(op->state == OP_PARSED);
    ck_assert(op->type = OP_QUIT);
    ck_assert(buf->rpos == buf->wpos);
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

    /* basic ops */
    TCase *tc_basic_op = tcase_create("basic op");
    suite_add_tcase(s, tc_basic_op);

    tcase_add_test(tc_basic_op, test_quit);

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
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

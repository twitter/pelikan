#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>

#include <cc_bstring.h>

#include <check.h>

#define SUITE_NAME "buffer"
#define DEBUG_LOG  SUITE_NAME ".log"

#define TEST_BUF_CAP                                  32
#define TEST_BUF_SIZE      (TEST_BUF_CAP + BUF_HDR_SIZE)
#define TEST_BUF_POOLSIZE                              0
#define TEST_DBUF_MAX                                  2

static buf_metrics_st bmetrics;
static dbuf_metrics_st dmetrics;

static buf_options_st boptions;
static dbuf_options_st doptions;

/*
 * utilities
 */
static void
test_setup(void)
{
    bmetrics = (buf_metrics_st) { BUF_METRIC(METRIC_INIT) };
    dmetrics = (dbuf_metrics_st) { DBUF_METRIC(METRIC_INIT) };

    boptions  = (buf_options_st){
        .buf_init_size = {
            .set = true,
            .type = OPTION_TYPE_UINT,
            .val.vuint = TEST_BUF_SIZE,
        },
        .buf_poolsize = {
            .set = true,
            .type = OPTION_TYPE_UINT,
            .val.vuint = TEST_BUF_POOLSIZE,
        }};

    doptions = (dbuf_options_st){
        .dbuf_max_power = {
            .set = true,
            .type = OPTION_TYPE_UINT,
            .val.vuint = TEST_DBUF_MAX,
        }};

    buf_setup(&boptions, &bmetrics);
    dbuf_setup(&doptions, &dmetrics);
}

static void
test_teardown(void)
{
    buf_teardown();
    dbuf_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

/*
 * tests
 */
START_TEST(test_create_write_read_destroy_basic)
{
#define MSG "Hello World"
#define NEW_CAP 100
    struct buf *buf = NULL;
    char message[sizeof(MSG)];

    test_reset();
    cc_memset(message, 0, sizeof(MSG));

    /* Test create and metrics */
    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);
    ck_assert_int_eq(bmetrics.buf_curr.gauge, 1);
    ck_assert_uint_eq(bmetrics.buf_create.counter, 1);
    ck_assert_uint_eq(bmetrics.buf_destroy.counter, 0);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, TEST_BUF_SIZE);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP);
    ck_assert_uint_eq(buf_size(buf), TEST_BUF_SIZE);
    ck_assert_uint_eq(buf_capacity(buf), TEST_BUF_CAP);
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP), NEW_CAP - TEST_BUF_CAP);

    /* Write message to buffer, and read, check buffer state */
    ck_assert_uint_eq(buf_write(buf, MSG, sizeof(MSG)), sizeof(MSG));
    ck_assert_uint_eq(buf_rsize(buf), sizeof(MSG));
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP - sizeof(MSG));
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP),
            NEW_CAP - (TEST_BUF_CAP - sizeof(MSG)));

    /* Read message from buffer, check buf state and if message is intact */
    ck_assert_uint_eq(buf_read(message, buf, sizeof(MSG)), sizeof(MSG));
    ck_assert_int_eq(cc_memcmp(message, MSG, sizeof(MSG)), 0);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP - sizeof(MSG));
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP),
            NEW_CAP - (TEST_BUF_CAP - sizeof(MSG)));

    /* Test destroy and metrics */
    buf_destroy(&buf);
    ck_assert_ptr_eq(buf, NULL);
    ck_assert_int_eq(bmetrics.buf_curr.gauge, 0);
    ck_assert_uint_eq(bmetrics.buf_create.counter, 1);
    ck_assert_uint_eq(bmetrics.buf_destroy.counter, 1);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, 0);
#undef MSG
#undef CAP
}
END_TEST

START_TEST(test_create_write_read_destroy_long)
{
#define MSG "this is a message that is long enough to fill up the entire buffer"
#define NEW_CAP 100
    struct buf *buf = NULL;
    char message[sizeof(MSG)];

    test_reset();
    cc_memset(message, 0, sizeof(MSG));

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    /* Write message to buffer, expect full buffer */
    ck_assert_uint_eq(buf_write(buf, MSG, sizeof(MSG)), TEST_BUF_CAP);
    ck_assert_uint_eq(buf_rsize(buf), TEST_BUF_CAP);
    ck_assert_uint_eq(buf_wsize(buf), 0);
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP), NEW_CAP);

    /* Read message from buffer, expect clipped message */
    ck_assert_uint_eq(buf_read(message, buf, sizeof(MSG)), TEST_BUF_CAP);
    ck_assert_int_eq(cc_memcmp(message, MSG, TEST_BUF_CAP), 0);
    ck_assert_int_ne(cc_memcmp(message, MSG, TEST_BUF_CAP + 1), 0);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), 0);
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP), NEW_CAP);

    buf_destroy(&buf);
#undef MSG
#undef NEW_CAP
}
END_TEST

START_TEST(test_lshift)
{
#define MSG "Hello World"
#define NEW_CAP 100
#define READ_LEN 5
    struct buf *buf = NULL;
    char message[sizeof(MSG)];

    test_reset();
    cc_memset(message, 0, sizeof(MSG));

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    /* Write message to buffer */
    ck_assert_uint_eq(buf_write(buf, MSG, sizeof(MSG)), sizeof(MSG));

    /* Read part of message */
    ck_assert_uint_eq(buf_read(message, buf, READ_LEN), READ_LEN);
    ck_assert_int_eq(cc_memcmp(message, MSG, READ_LEN), 0);
    ck_assert_int_ne(cc_memcmp(message, MSG, READ_LEN + 1), 0);
    ck_assert_uint_eq(buf_rsize(buf), sizeof(MSG) - READ_LEN);
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP - sizeof(MSG));
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP),
            NEW_CAP - (TEST_BUF_CAP - sizeof(MSG)));

    /* lshift buffer, check state */
    buf_lshift(buf);
    ck_assert_uint_eq(buf_rsize(buf), sizeof(MSG) - READ_LEN);
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP - (sizeof(MSG) - READ_LEN));
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP),
            NEW_CAP - (TEST_BUF_CAP - (sizeof(MSG) - READ_LEN)));

    /* Read rest of message */
    ck_assert_uint_eq(buf_read(message + READ_LEN, buf, sizeof(MSG)),
            sizeof(MSG) - READ_LEN);
    ck_assert_int_eq(cc_memcmp(message, MSG, sizeof(MSG)), 0);
    ck_assert_uint_eq(buf_rsize(buf), 0);

    /* lshift again */
    buf_lshift(buf);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP);
    ck_assert_uint_eq(buf_size(buf), TEST_BUF_SIZE);
    ck_assert_uint_eq(buf_capacity(buf), TEST_BUF_CAP);
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP), NEW_CAP - TEST_BUF_CAP);

    buf_destroy(&buf);
#undef MSG
#undef NEW_CAP
#undef READ_LEN
}
END_TEST

START_TEST(test_rshift)
{
#define MSG "Hello World"
#define NEW_CAP 100
#define READ_LEN 5
    struct buf *buf = NULL;
    char message[sizeof(MSG)];

    test_reset();
    cc_memset(message, 0, sizeof(MSG));

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    /* Write message to buffer */
    ck_assert_uint_eq(buf_write(buf, MSG, sizeof(MSG)), sizeof(MSG));

    /* Read part of message */
    ck_assert_uint_eq(buf_read(message, buf, READ_LEN), READ_LEN);

    /* rshift buffer, check state */
    buf_rshift(buf);
    ck_assert_uint_eq(buf_rsize(buf), sizeof(MSG) - READ_LEN);
    ck_assert_uint_eq(buf_wsize(buf), 0);
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP), NEW_CAP);

    /* Read rest of message */
    ck_assert_uint_eq(buf_read(message + READ_LEN, buf, sizeof(MSG)),
            sizeof(MSG) - READ_LEN);
    ck_assert_int_eq(cc_memcmp(message, MSG, sizeof(MSG)), 0);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), 0);

    buf_destroy(&buf);
#undef MSG
#undef NEW_CAP
#undef READ_LEN
}
END_TEST

START_TEST(test_dbuf_double_basic)
{
#define EXPECTED_BUF_SIZE                (TEST_BUF_SIZE * 2)
#define EXPECTED_BUF_CAP  (EXPECTED_BUF_SIZE - BUF_HDR_SIZE)
#define NEW_CAP                                          200
    struct buf *buf;

    test_reset();

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    /* double buffer, check state */
    ck_assert_int_eq(dbuf_double(&buf), CC_OK);
    ck_assert_ptr_ne(buf, NULL);
    ck_assert_int_eq(bmetrics.buf_curr.gauge, 1);
    ck_assert_uint_eq(bmetrics.buf_create.counter, 1);
    ck_assert_uint_eq(bmetrics.buf_destroy.counter, 0);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, EXPECTED_BUF_SIZE);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), EXPECTED_BUF_CAP);
    ck_assert_uint_eq(buf_size(buf), EXPECTED_BUF_SIZE);
    ck_assert_uint_eq(buf_capacity(buf), EXPECTED_BUF_CAP);
    ck_assert_uint_eq(buf_new_cap(buf, NEW_CAP), NEW_CAP - EXPECTED_BUF_CAP);

    /* destroy, check if memory gauge decremented correctly */
    buf_destroy(&buf);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, 0);
#undef EXPECTED_BUF_SIZE
#undef EXPECTED_BUF_CAP
#undef NEW_CAP
}
END_TEST

START_TEST(test_dbuf_double_over_max)
{
    int i;
    struct buf *buf;

    test_reset();

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    for (i = 0; i < TEST_DBUF_MAX; ++i) {
        ck_assert_int_eq(dbuf_double(&buf), CC_OK);
    }

    ck_assert_int_eq(dbuf_double(&buf), CC_ERROR);

    buf_destroy(&buf);
}
END_TEST

START_TEST(test_dbuf_fit)
{
#define CAP_SMALL                         (TEST_BUF_CAP * 4)
#define EXPECTED_BUF_SIZE                (TEST_BUF_SIZE * 4)
#define EXPECTED_BUF_CAP  (EXPECTED_BUF_SIZE - BUF_HDR_SIZE)
#define CAP_LARGE (TEST_BUF_CAP * 16)
    struct buf *buf;

    test_reset();

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    /* fit to small size, check state */
    ck_assert_int_eq(dbuf_fit(&buf, CAP_SMALL), CC_OK);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, EXPECTED_BUF_SIZE);
    ck_assert_uint_eq(buf_rsize(buf), 0);
    ck_assert_uint_eq(buf_wsize(buf), EXPECTED_BUF_CAP);
    ck_assert_uint_eq(buf_size(buf), EXPECTED_BUF_SIZE);
    ck_assert_uint_eq(buf_capacity(buf), EXPECTED_BUF_CAP);

    /* attempt to fit to large size */
    ck_assert_int_eq(dbuf_fit(&buf, CAP_LARGE), CC_ERROR);

    buf_destroy(&buf);
#undef CAP_SMALL
#undef EXPECTED_BUF_SIZE
#undef EXPECTED_BUF_CAP
#undef CAP_LARGE
}
END_TEST

START_TEST(test_dbuf_shrink)
{
#define MSG1 "Hello World"
#define MSG2 "this message can be contained by a singly doubled buffer"
#define EXPECTED_BUF_SIZE                (TEST_BUF_SIZE * 2)
#define EXPECTED_BUF_CAP  (EXPECTED_BUF_SIZE - BUF_HDR_SIZE)
    struct buf *buf;

    test_reset();

    buf = buf_create();
    ck_assert_ptr_ne(buf, NULL);

    /* write first message, double twice, then shrink */
    ck_assert_uint_eq(buf_write(buf, MSG1, sizeof(MSG1)), sizeof(MSG1));
    ck_assert_int_eq(dbuf_double(&buf), CC_OK);
    ck_assert_int_eq(dbuf_double(&buf), CC_OK);

    /* shrink, then check state */
    ck_assert_int_eq(dbuf_shrink(&buf), CC_OK);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, TEST_BUF_SIZE);
    ck_assert_uint_eq(buf_rsize(buf), sizeof(MSG1));
    ck_assert_uint_eq(buf_wsize(buf), TEST_BUF_CAP - sizeof(MSG1));
    ck_assert_uint_eq(buf_size(buf), TEST_BUF_SIZE);
    ck_assert_uint_eq(buf_capacity(buf), TEST_BUF_CAP);

    buf_reset(buf);

    /* double twice, then write second message */
    ck_assert_int_eq(dbuf_double(&buf), CC_OK);
    ck_assert_int_eq(dbuf_double(&buf), CC_OK);
    ck_assert_uint_eq(buf_write(buf, MSG2, sizeof(MSG2)), sizeof(MSG2));

    /* shrink, then check state */
    ck_assert_int_eq(dbuf_shrink(&buf), CC_OK);
    ck_assert_int_eq(bmetrics.buf_memory.gauge, EXPECTED_BUF_SIZE);
    ck_assert_uint_eq(buf_rsize(buf), sizeof(MSG2));
    ck_assert_uint_eq(buf_wsize(buf), EXPECTED_BUF_CAP - sizeof(MSG2));
    ck_assert_uint_eq(buf_size(buf), EXPECTED_BUF_SIZE);
    ck_assert_uint_eq(buf_capacity(buf), EXPECTED_BUF_CAP);

    buf_destroy(&buf);
#undef MSG1
#undef MSG2
#undef EXPECTED_BUF_SIZE
#undef EXPECTED_BUF_CAP
}
END_TEST

/*
 * test suite
 */
static Suite *
buf_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_buf = tcase_create("buf test");
    suite_add_tcase(s, tc_buf);

    tcase_add_test(tc_buf, test_create_write_read_destroy_basic);
    tcase_add_test(tc_buf, test_create_write_read_destroy_long);
    tcase_add_test(tc_buf, test_lshift);
    tcase_add_test(tc_buf, test_rshift);

    TCase *tc_dbuf = tcase_create("dbuf test");
    suite_add_tcase(s, tc_dbuf);

    tcase_add_test(tc_dbuf, test_dbuf_double_basic);
    tcase_add_test(tc_dbuf, test_dbuf_double_over_max);
    tcase_add_test(tc_dbuf, test_dbuf_fit);
    tcase_add_test(tc_dbuf, test_dbuf_shrink);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = buf_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

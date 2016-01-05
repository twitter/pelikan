#include <cc_rbuf.h>

#include <check.h>

#include <limits.h>
#include <stdlib.h>
#include <stdio.h>

#define SUITE_NAME "rbuf"
#define DEBUG_LOG  SUITE_NAME ".log"

#define ARRAY_MAX_NELEM_DELTA 8

/*
 * utilities
 */
static void
test_setup(void)
{
    rbuf_setup(NULL);
}

static void
test_teardown(void)
{
    rbuf_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

static void
write_read_rbuf(struct rbuf *buffer, char *write_data, size_t w1_len, size_t w2_len)
{
    char *read_data;
    size_t cap, written, read;

    cap = w1_len + w2_len;
    read_data = malloc(sizeof(char) * cap);
    ck_assert_ptr_ne(read_data, NULL);

    written = rbuf_write(buffer, write_data, w1_len);
    ck_assert_int_eq(written, w1_len);

    ck_assert_int_eq(rbuf_rcap(buffer), w1_len);
    ck_assert_int_eq(rbuf_wcap(buffer), w2_len);


    written = rbuf_write(buffer, &write_data[w1_len], w2_len);
    ck_assert_int_eq(written, w2_len);

    ck_assert_int_eq(rbuf_rcap(buffer), cap);
    ck_assert_int_eq(rbuf_wcap(buffer), 0);

    read = rbuf_read(read_data, buffer, w1_len);
    ck_assert_int_eq(read, w1_len);

    read = rbuf_read(&read_data[w1_len], buffer, w2_len);
    ck_assert_int_eq(read, w2_len);

    ck_assert_int_eq(memcmp(read_data, write_data, cap), 0);

    free(read_data);
}

START_TEST(test_create_write_read_destroy)
{
#define W1_LEN 8
#define W2_LEN 12
#define CAP (W1_LEN + W2_LEN)
    size_t i;
    char write_data[CAP];
    struct rbuf *buffer;

    test_reset();

    for (i = 0; i < CAP; i++) {
        write_data[i] = i % CHAR_MAX;
    }

    buffer = rbuf_create(CAP);
    ck_assert_ptr_ne(buffer, NULL);

    write_read_rbuf(buffer, write_data, W1_LEN, W2_LEN);

    rbuf_destroy(&buffer);
#undef CAP
#undef W2_LEN
#undef W1_LEN
}
END_TEST

START_TEST(test_create_write_read_wrap_around_destroy)
{
#define W1_LEN 8
#define W2_LEN 12
#define CAP (W1_LEN + W2_LEN)
    size_t i, written, read;
    char write_data[CAP], read_data[CAP];
    struct rbuf *buffer;

    test_reset();

    for (i = 0; i < CAP; i++) {
        write_data[i] = i % CHAR_MAX;
    }

    buffer = rbuf_create(CAP);
    ck_assert_ptr_ne(buffer, NULL);

    /* writting and reading to force a wrap around */
    written = rbuf_write(buffer, write_data, CAP - 1);
    ck_assert_int_eq(written, CAP - 1);
    read = rbuf_read(read_data, buffer, CAP - 1);
    ck_assert_int_eq(read, CAP - 1);

    write_read_rbuf(buffer, write_data, W1_LEN, W2_LEN);

    rbuf_destroy(&buffer);
#undef CAP
#undef W2_LEN
#undef W1_LEN
}
END_TEST

/*
 * test suite
 */
static Suite *
rbuf_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic requests */
    TCase *tc_rbuf = tcase_create("cc_rbuf test");
    suite_add_tcase(s, tc_rbuf);

    tcase_add_test(tc_rbuf, test_create_write_read_destroy);
    tcase_add_test(tc_rbuf, test_create_write_read_wrap_around_destroy);

    return s;
}
/**************
 * test cases *
 **************/

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = rbuf_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

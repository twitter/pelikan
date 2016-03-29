#include <cc_event.h>
#include <channel/cc_pipe.h>

#include <check.h>

#include <fcntl.h>
#include <stdlib.h>
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>

#define SUITE_NAME "event"
#define DEBUG_LOG  SUITE_NAME ".log"

struct event {
    void *arg;
    uint32_t events;
};

static struct event event_log[1024];
static uint32_t event_log_count;

/*
 * utilities
 */
static void
test_setup(void)
{
    event_log_count = 0;
    event_setup(NULL);
}

static void
test_teardown(void)
{
    event_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

static void
log_event(void *arg, uint32_t events)
{
    event_log[event_log_count].arg = arg;
    event_log[event_log_count++].events = events;
}

START_TEST(test_read)
{
#define DATA "foo bar baz"
    struct event_base *event_base;
    int random_pointer[1] = {1};
    struct pipe_conn *pipe;

    test_reset();

    event_base = event_base_create(1024, log_event);

    pipe = pipe_conn_create();
    ck_assert_int_eq(pipe_open(NULL, pipe), true);
    ck_assert_int_eq(pipe_send(pipe, DATA, sizeof(DATA)), sizeof(DATA));

    event_add_read(event_base, pipe_read_id(pipe), random_pointer);

    ck_assert_int_eq(event_log_count, 0);

    event_wait(event_base, -1);

    ck_assert_int_eq(event_log_count, 1);
    ck_assert_ptr_eq(event_log[0].arg, random_pointer);
    ck_assert_int_eq(event_log[0].events, EVENT_READ);

    ck_assert_int_eq(event_del(event_base, pipe_read_id(pipe)), 0);
    event_base_destroy(&event_base);
    pipe_close(pipe);
    pipe_conn_destroy(&pipe);
#undef DATA
}
END_TEST

START_TEST(test_cannot_read)
{
    struct event_base *event_base;
    int random_pointer[1] = {1};
    struct pipe_conn *pipe;

    test_reset();

    event_base = event_base_create(1024, log_event);

    pipe = pipe_conn_create();
    ck_assert_int_eq(pipe_open(NULL, pipe), true);

    event_add_read(event_base, pipe_read_id(pipe), random_pointer);

    ck_assert_int_eq(event_log_count, 0);

    event_wait(event_base, 1000);

    ck_assert_int_eq(event_log_count, 0);

    ck_assert_int_eq(event_del(event_base, pipe_read_id(pipe)), 0);
    event_base_destroy(&event_base);
    pipe_close(pipe);
    pipe_conn_destroy(&pipe);
}
END_TEST

START_TEST(test_write)
{
    struct event_base *event_base;
    int random_pointer[1] = {1};
    struct pipe_conn *pipe;

    test_reset();

    event_base = event_base_create(1024, log_event);

    pipe = pipe_conn_create();
    ck_assert_int_eq(pipe_open(NULL, pipe), true);

    event_add_write(event_base, pipe_write_id(pipe), random_pointer);

    ck_assert_int_eq(event_log_count, 0);

    event_wait(event_base, -1);

    ck_assert_int_eq(event_log_count, 1);
    ck_assert_ptr_eq(event_log[0].arg, random_pointer);
    ck_assert_int_eq(event_log[0].events, EVENT_WRITE);

    ck_assert_int_eq(event_del(event_base, pipe_write_id(pipe)), 0);
    event_base_destroy(&event_base);
    pipe_close(pipe);
    pipe_conn_destroy(&pipe);
}
END_TEST

/*
 * test suite
 */
static Suite *
event_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic requests */
    TCase *tc_event = tcase_create("cc_event test");
    suite_add_tcase(s, tc_event);

    tcase_add_test(tc_event, test_read);
    tcase_add_test(tc_event, test_cannot_read);
    tcase_add_test(tc_event, test_write);

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

    Suite *suite = event_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

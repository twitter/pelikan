#include <channel/cc_pipe.h>
#include <time/cc_timer.h>

#include <check.h>

#include <pthread.h>
#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>

#define SUITE_NAME "pipe"
#define DEBUG_LOG  SUITE_NAME ".log"

struct write_task {
    struct pipe_conn *pipe;
    void *buf;
    size_t nbytes;
    useconds_t usleep;
};

/*
 * utilities
 */
static void
test_setup(void)
{
    pipe_setup(NULL, NULL);
}

static void
test_teardown(void)
{
    pipe_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

static void *do_write(void *_write_task)
{
    struct write_task* task = _write_task;
    if (task->usleep) {
        usleep(task->usleep);
    }
    ck_assert_int_eq(pipe_send(task->pipe, task->buf, task->nbytes), task->nbytes);
    return NULL;
}

START_TEST(test_send_recv)
{
    struct pipe_conn *pipe;
    const char *write_message = "foo bar baz";
    struct write_task task;
#define READ_MESSAGE_LENGTH 12
    char read_message[READ_MESSAGE_LENGTH];
    test_reset();

    pipe = pipe_conn_create();
    ck_assert_ptr_ne(pipe, NULL);

    ck_assert_int_eq(pipe_open(NULL, pipe), true);

    task.pipe = pipe;
    task.buf = (void *)write_message;
    task.nbytes = READ_MESSAGE_LENGTH;
    task.usleep = 0;
    do_write(&task);

    ck_assert_int_eq(pipe_recv(pipe, read_message, READ_MESSAGE_LENGTH), READ_MESSAGE_LENGTH);

    ck_assert_str_eq(write_message, read_message);

    pipe_close(pipe);
    pipe_conn_destroy(&pipe);
#undef READ_MESSAGE_LENGTH
}
END_TEST

START_TEST(test_read_blocking)
{
    struct pipe_conn *pipe;
    const char *write_message = "foo bar baz";
    struct duration duration;
    struct write_task task;
    pthread_t thread;
#define READ_MESSAGE_LENGTH 12
#define SLEEP_TIME 500000
#define TOLERANCE_TIME 100000
    char read_message[READ_MESSAGE_LENGTH];
    test_reset();
    duration_reset(&duration);

    pipe = pipe_conn_create();
    ck_assert_ptr_ne(pipe, NULL);

    ck_assert_int_eq(pipe_open(NULL, pipe), true);

    duration_start(&duration);

    task.pipe = pipe;
    task.buf = (void *)write_message;
    task.nbytes = READ_MESSAGE_LENGTH;
    task.usleep = SLEEP_TIME;
    pthread_create(&thread, NULL, do_write, &task);
    ck_assert_int_eq(pipe_recv(pipe, read_message, READ_MESSAGE_LENGTH), READ_MESSAGE_LENGTH);

    duration_stop(&duration);
    pthread_join(thread, NULL);

    ck_assert_int_ge(duration_us(&duration), SLEEP_TIME);
    ck_assert_int_le(duration_us(&duration), SLEEP_TIME + TOLERANCE_TIME);

    ck_assert_str_eq(write_message, read_message);

    pipe_close(pipe);
    pipe_conn_destroy(&pipe);
#undef READ_MESSAGE_LENGTH
#undef SLEEP_TIME
#undef TOLERANCE_TIME
}
END_TEST

START_TEST(test_read_nonblocking)
{
    struct pipe_conn *pipe;
    const char *write_message = "foo bar baz";
    struct write_task task;
#define READ_MESSAGE_LENGTH 12
    char read_message[READ_MESSAGE_LENGTH];
    test_reset();

    pipe = pipe_conn_create();
    ck_assert_ptr_ne(pipe, NULL);

    ck_assert_int_eq(pipe_open(NULL, pipe), true);
    pipe_set_nonblocking(pipe);

    ck_assert_int_lt(pipe_recv(pipe, read_message, READ_MESSAGE_LENGTH), 0);
    task.pipe = pipe;
    task.buf = (void *)write_message;
    task.nbytes = READ_MESSAGE_LENGTH;
    task.usleep = 0;
    do_write(&task);

    ck_assert_int_eq(pipe_recv(pipe, read_message, READ_MESSAGE_LENGTH), READ_MESSAGE_LENGTH);

    ck_assert_str_eq(write_message, read_message);

    pipe_close(pipe);
    pipe_conn_destroy(&pipe);
#undef READ_MESSAGE_LENGTH
}
END_TEST

/*
 * test suite
 */
static Suite *
pipe_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_pipe = tcase_create("pipe test");
    tcase_add_test(tc_pipe, test_send_recv);
    tcase_add_test(tc_pipe, test_read_blocking);
    tcase_add_test(tc_pipe, test_read_nonblocking);
    suite_add_tcase(s, tc_pipe);

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

    Suite *suite = pipe_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

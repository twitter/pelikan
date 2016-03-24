#include <channel/cc_tcp.h>

#include <check.h>

#include <netdb.h>
#include <sys/types.h>
#include <sys/socket.h>

#define SUITE_NAME "tcp"
#define DEBUG_LOG  SUITE_NAME ".log"
#define HOST NULL
#define PORT "12321"

static struct addrinfo *ai;
static struct tcp_conn *server;
static struct tcp_conn *client;

/*
 * utilities
 */
static int
get_addr(struct addrinfo **ai_ptr)
{
    struct addrinfo hints = { .ai_flags = AI_PASSIVE, .ai_family = AF_UNSPEC,
                              .ai_socktype = SOCK_STREAM };
    return getaddrinfo(HOST, PORT, &hints, ai_ptr);
}

static void
test_setup(void)
{
    tcp_setup(NULL, NULL);
    if (get_addr(&ai) != 0) {
        exit(EXIT_FAILURE);
    }
    server = tcp_conn_create();
    client = tcp_conn_create();
}

static void
test_teardown(void)
{
    tcp_teardown();
    if (ai != NULL) {
        freeaddrinfo(ai);
        ai = NULL;
    }
    tcp_conn_destroy(&server);
    tcp_conn_destroy(&client);
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

START_TEST(test_accept)
{
    struct tcp_conn *c = tcp_conn_create();
    cc_memset(c->peer, '\0', INET6_ADDRSTRLEN + 1 + 5 + 1);

    test_reset();

    ck_assert(tcp_listen(ai, server));
    ck_assert(tcp_connect(ai, client));
    ck_assert(tcp_accept(server, c));
    ck_assert(c->free == false);
    ck_assert_int_gt(c->sd, 0);
    ck_assert_msg(strnlen(c->peer, INET6_ADDRSTRLEN + 1 + 5 + 1) > 0, "c->peer was not set");
    tcp_close(c);
    tcp_close(client);
    tcp_close(server);
}
END_TEST

/*
 * test suite
 */
static Suite *
tcp_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_tcp = tcase_create("tcp test");
    tcase_add_test(tc_tcp, test_accept);
    suite_add_tcase(s, tc_tcp);

    return s;
}

int
main(void)
{
    int nfail = 0;

    /* setup */
    test_setup();

    Suite *suite = tcp_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV);
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

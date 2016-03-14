#include <cc_option.h>

#include <check.h>

#include <math.h>
#include <stdlib.h>
#include <stdio.h>

#define SUITE_NAME "option"
#define DEBUG_LOG  SUITE_NAME ".log"

/*
 * utilities
 */
static void
test_setup(void)
{
}

static void
test_teardown(void)
{
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

START_TEST(test_parse_bool)
{
    struct option opt;
    opt.type = OPTION_TYPE_BOOL;
    opt.set = false;

    ck_assert_int_ne(option_set(&opt, "invalid"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    opt.set = false;
    opt.val.vbool = false;
    ck_assert_int_eq(option_set(&opt, "yes"), CC_OK);
    ck_assert_int_eq(opt.val.vbool, true);
    ck_assert_int_eq(opt.set, true);

    opt.set = false;
    opt.val.vbool = true;
    ck_assert_int_eq(option_set(&opt, "no"), CC_OK);
    ck_assert_int_eq(opt.val.vbool, false);
    ck_assert_int_eq(opt.set, true);
}
END_TEST

START_TEST(test_parse_uinteger)
{
    struct option opt;
    opt.type = OPTION_TYPE_UINT;
    opt.set = false;

    ck_assert_int_ne(option_set(&opt, "invalid"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    ck_assert_int_ne(option_set(&opt, "-1"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    ck_assert_int_ne(option_set(&opt, "0 - 1"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    ck_assert_int_ne(option_set(&opt, "(1 + 2"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    ck_assert_int_ne(option_set(&opt, "1 + 2)"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    opt.set = false;
    opt.val.vuint = 0;
    ck_assert_int_eq(option_set(&opt, "1"), CC_OK);
    ck_assert_int_eq(opt.val.vuint, 1);
    ck_assert_int_eq(opt.set, true);

    opt.set = false;
    opt.val.vuint = 0;
    ck_assert_int_eq(option_set(&opt, "1 + 1"), CC_OK);
    ck_assert_int_eq(opt.val.vuint, 2);
    ck_assert_int_eq(opt.set, true);

    opt.set = false;
    opt.val.vuint = 0;
    ck_assert_int_eq(option_set(&opt, "1 + 2 * 3"), CC_OK);
    ck_assert_int_eq(opt.val.vuint, 7);
    ck_assert_int_eq(opt.set, true);

    opt.set = false;
    opt.val.vuint = 0;
    ck_assert_int_eq(option_set(&opt, "(1 + 2) * 3"), CC_OK);
    ck_assert_int_eq(opt.val.vuint, 9);
    ck_assert_int_eq(opt.set, true);
}
END_TEST

START_TEST(test_parse_float)
{
    struct option opt;
    opt.type = OPTION_TYPE_FPN;
    opt.set = false;

    ck_assert_int_ne(option_set(&opt, "invalid"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    ck_assert_int_ne(option_set(&opt, "1.25ab"), CC_OK);
    ck_assert_int_eq(opt.set, false);

    opt.set = false;
    opt.val.vfpn = 0;
    ck_assert_int_eq(option_set(&opt, "1.25"), CC_OK);
    ck_assert_msg(fabs(opt.val.vfpn - 1.25) < 1e-5, "value = %f", opt.val.vfpn);
    ck_assert_int_eq(opt.set, true);

    opt.set = false;
    opt.val.vfpn = 0;
    ck_assert_int_eq(option_set(&opt, "-1"), CC_OK);
    ck_assert(fabs(opt.val.vfpn + 1) < 1e-5);
    ck_assert_int_eq(opt.set, true);
}
END_TEST

START_TEST(test_parse_string)
{
    struct option opt;
    opt.type = OPTION_TYPE_STR;
    opt.val.vstr = NULL;

    opt.set = false;
    opt.val.vstr = NULL;
    ck_assert_int_eq(option_set(&opt, "1"), CC_OK);
    ck_assert_str_eq(opt.val.vstr, "1");
    ck_assert_int_eq(opt.set, true);
    option_free(&opt, 1);

    opt.set = false;
    opt.val.vstr = NULL;
    ck_assert_int_eq(option_set(&opt, "a\nb"), CC_OK);
    ck_assert_str_eq(opt.val.vstr, "a\nb");
    ck_assert_int_eq(opt.set, true);
    option_free(&opt, 1);
}
END_TEST

static char *
tmpname_create(char *data)
{
#define PATH "/tmp/temp.XXXXXX"
    size_t datalen = strlen(data);

    char *path = malloc(sizeof(PATH) + 3);
    strcpy(path, PATH);
    mkdtemp(path);
    size_t len = strlen(path);
    path[len++] = '/';
    path[len++] = '1';
    path[len++] = 0;

    FILE *fp = fopen(path, "w");
    ck_assert_uint_eq(fwrite(data, 1, datalen, fp), datalen);
    fclose(fp);

    return path;
#undef PATH
}

static void
tmpname_destroy(char *path)
{
    unlink(path);
    path[strlen(path) - 2] = 0;
    rmdir(path);
    free(path);
}

START_TEST(test_load_file)
{
#define TEST_OPTION(ACTION)                                                                 \
    ACTION( boolean,    OPTION_TYPE_BOOL,   true,   "it may be true of false"              )\
    ACTION( uinteger,   OPTION_TYPE_UINT,   2,      "it is a non-negative integer number"  )\
    ACTION( fpn,        OPTION_TYPE_FPN,    1.25,   "it is a floating point number"        )\
    ACTION( string,     OPTION_TYPE_STR,    "foo",  "it is a sequence of bytes"            )

#define SETTING(ACTION) \
    TEST_OPTION(ACTION)

    struct setting {
        SETTING(OPTION_DECLARE)
    } setting = {
        SETTING(OPTION_INIT)
    };
    const unsigned int nopt = OPTION_CARDINALITY(struct setting);

    char *tmpname;
    FILE *fp;

    test_reset();

    tmpname = tmpname_create(
        "boolean: no\n"
        "uinteger: 3\n"
        "fpn:    2.5\n"
        "string: bar\n"
    );
    ck_assert_ptr_ne(tmpname, NULL);

    fp = fopen(tmpname, "r");
    ck_assert_ptr_ne(fp, NULL);

    ck_assert_int_eq(option_load_default((struct option *)&setting, nopt),
            CC_OK);
    ck_assert_int_eq(setting.boolean.val.vbool, true);
    ck_assert_int_eq(setting.uinteger.val.vuint, 2);
    ck_assert_msg(fabs(setting.fpn.val.vfpn - 1.25) < 1e-5, "value = %f",
            setting.fpn.val.vfpn);
    ck_assert_str_eq(setting.string.val.vstr, "foo");

    ck_assert_int_eq(option_load_file(fp, (struct option *)&setting, nopt),
            CC_OK);
    ck_assert_int_eq(setting.boolean.val.vbool, false);
    ck_assert_int_eq(setting.uinteger.val.vuint, 3);
    ck_assert(fabs(setting.fpn.val.vfpn - 2.5) < 1e-5);
    ck_assert_str_eq(setting.string.val.vstr, "bar");

    option_free((struct option *)&setting, nopt);
    fclose(fp);

    tmpname_destroy(tmpname);
#undef TEST_OPTION
#undef SETTING
}
END_TEST

/*
 * test suite
 */
static Suite *
option_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_option = tcase_create("option test");
    tcase_add_test(tc_option, test_parse_bool);
    tcase_add_test(tc_option, test_parse_uinteger);
    tcase_add_test(tc_option, test_parse_float);
    tcase_add_test(tc_option, test_parse_string);
    tcase_add_test(tc_option, test_load_file);
    suite_add_tcase(s, tc_option);

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

    Suite *suite = option_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}

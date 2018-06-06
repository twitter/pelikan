#pragma once

/*          name        type            description */
#define PROCESS_LIST_METRIC(ACTION)                                             \
    ACTION( list_create,            METRIC_COUNTER, "# list create requests"   )\
    ACTION( list_create_exist,      METRIC_COUNTER, "# list already exist"     )\
    ACTION( list_create_stored,     METRIC_COUNTER, "# list stored"            )\
    ACTION( list_create_ex,         METRIC_COUNTER, "# list create exceptions" )\
    ACTION( list_delete,            METRIC_COUNTER, "# list delete requests"   )\
    ACTION( list_delete_deleted,    METRIC_COUNTER, "# list delete success"    )\
    ACTION( list_delete_notfound,   METRIC_COUNTER, "# list delete success"    )\
    ACTION( list_trim,              METRIC_COUNTER, "# list trim requests"     )\
    ACTION( list_len,               METRIC_COUNTER, "# list length requests"   )\
    ACTION( list_find,              METRIC_COUNTER, "# list find requests"     )\
    ACTION( list_get,               METRIC_COUNTER, "# list get requests"      )\
    ACTION( list_insert,            METRIC_COUNTER, "# list insert requests"   )\
    ACTION( list_push,              METRIC_COUNTER, "# list push requests"     )

struct request;
struct response;
struct command;

/* cmd_* functions must be command_fn (process.c) compatible */
void cmd_list_create(struct response *rsp, struct request *req, struct command *cmd);
void cmd_list_delete(struct response *rsp, struct request *req, struct command *cmd);

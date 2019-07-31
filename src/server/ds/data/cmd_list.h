#pragma once

/*          name        type            description */
#define PROCESS_LIST_METRIC(ACTION)                                                 \
    ACTION( list_create,            METRIC_COUNTER, "# list create requests"       )\
    ACTION( list_create_exist,      METRIC_COUNTER, "# list already exist"         )\
    ACTION( list_create_stored,     METRIC_COUNTER, "# list stored"                )\
    ACTION( list_create_ex,         METRIC_COUNTER, "# list create exceptions"     )\
    ACTION( list_delete,            METRIC_COUNTER, "# list delete requests"       )\
    ACTION( list_delete_deleted,    METRIC_COUNTER, "# list delete success"        )\
    ACTION( list_delete_notfound,   METRIC_COUNTER, "# list delete miss"           )\
    ACTION( list_trim,              METRIC_COUNTER, "# list trim requests"         )\
    ACTION( list_trim_notfound,     METRIC_COUNTER, "# list trim miss"             )\
    ACTION( list_trim_oob,          METRIC_COUNTER, "# list trim out of bound"     )\
    ACTION( list_len,               METRIC_COUNTER, "# list length requests"       )\
    ACTION( list_len_notfound,      METRIC_COUNTER, "# list length miss"           )\
    ACTION( list_find,              METRIC_COUNTER, "# list find requests"         )\
    ACTION( list_get,               METRIC_COUNTER, "# list get requests"          )\
    ACTION( list_get_notfound,      METRIC_COUNTER, "# list get miss"              )\
    ACTION( list_get_oob,           METRIC_COUNTER, "# list get out of bound"      )\
    ACTION( list_insert,            METRIC_COUNTER, "# list insert requests"       )\
    ACTION( list_insert_notfound,   METRIC_COUNTER, "# list insert miss"           )\
    ACTION( list_insert_oob,        METRIC_COUNTER, "# list insert out of bound"   )\
    ACTION( list_insert_ex,         METRIC_COUNTER, "# list insert exceptions"     )\
    ACTION( list_push,              METRIC_COUNTER, "# list push requests"         )\
    ACTION( list_push_notfound,     METRIC_COUNTER, "# list push miss"             )\
    ACTION( list_push_ex,           METRIC_COUNTER, "# list push exceptions"       )

struct request;
struct response;
struct command;

/* cmd_* functions must be command_fn (process.c) compatible */
void cmd_list_create(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_delete(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_trim(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_len(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_find(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_get(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_insert(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_list_push(struct response *rsp, const struct request *req, const struct command *cmd);

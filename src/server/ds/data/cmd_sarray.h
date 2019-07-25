#pragma once

/*          name                        type            description */
#define PROCESS_SARRAY_METRIC(ACTION)                                                 \
    ACTION( sarray_create,              METRIC_COUNTER, "# sarray create requests"       )\
    ACTION( sarray_create_exist,        METRIC_COUNTER, "# sarray already exist"         )\
    ACTION( sarray_create_stored,       METRIC_COUNTER, "# sarray stored"                )\
    ACTION( sarray_create_ex,           METRIC_COUNTER, "# sarray create exceptions"     )\
    ACTION( sarray_delete,              METRIC_COUNTER, "# sarray delete requests"       )\
    ACTION( sarray_delete_deleted,      METRIC_COUNTER, "# sarray delete success"        )\
    ACTION( sarray_delete_notfound,     METRIC_COUNTER, "# sarray delete miss"           )\
    ACTION( sarray_len,                 METRIC_COUNTER, "# sarray length requests"       )\
    ACTION( sarray_len_notfound,        METRIC_COUNTER, "# sarray length miss"           )\
    ACTION( sarray_find,                METRIC_COUNTER, "# sarray find requests"         )\
    ACTION( sarray_find_notfound,       METRIC_COUNTER, "# sarray find miss"             )\
    ACTION( sarray_get,                 METRIC_COUNTER, "# sarray get requests"          )\
    ACTION( sarray_get_notfound,        METRIC_COUNTER, "# sarray get miss"              )\
    ACTION( sarray_get_oob,             METRIC_COUNTER, "# sarray get out of bound"      )\
    ACTION( sarray_insert,              METRIC_COUNTER, "# sarray insert requests"       )\
    ACTION( sarray_insert_notfound,     METRIC_COUNTER, "# sarray insert miss"           )\
    ACTION( sarray_insert_noop,         METRIC_COUNTER, "# sarray insert out of bound"   )\
    ACTION( sarray_insert_ex,           METRIC_COUNTER, "# sarray insert exceptions"     )\
    ACTION( sarray_remove,              METRIC_COUNTER, "# sarray remove requests"       )\
    ACTION( sarray_remove_notfound,     METRIC_COUNTER, "# sarray remove miss"           )\
    ACTION( sarray_remove_noop,         METRIC_COUNTER, "# sarray remove no-op"          )\
    ACTION( sarray_remove_ex,           METRIC_COUNTER, "# sarray remove exceptions"     )\
    ACTION( sarray_truncate,            METRIC_COUNTER, "# sarray truncate requests"     )\
    ACTION( sarray_truncate_notfound,   METRIC_COUNTER, "# sarray truncate miss"         )

struct request;
struct response;
struct command;

/* cmd_* functions must be command_fn (process.c) compatible */
void cmd_sarray_create(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_delete(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_truncate(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_len(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_find(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_get(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_insert(struct response *rsp, struct request *req, struct command *cmd);
void cmd_sarray_remove(struct response *rsp, struct request *req, struct command *cmd);

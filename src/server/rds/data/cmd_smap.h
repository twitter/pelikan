#pragma once

/*          name                    type            description */
#define PROCESS_SMAP_METRIC(ACTION)                                                 \
    ACTION( smap_create,            METRIC_COUNTER, "# smap create requests"       )\
    ACTION( smap_create_exist,      METRIC_COUNTER, "# smap already exist"         )\
    ACTION( smap_create_ok,         METRIC_COUNTER, "# smap stored"                )\
    ACTION( smap_create_ex,         METRIC_COUNTER, "# smap create exceptions"     )\
    ACTION( smap_delete,            METRIC_COUNTER, "# smap delete requests"       )\
    ACTION( smap_delete_ok,         METRIC_COUNTER, "# smap delete success"        )\
    ACTION( smap_delete_notfound,   METRIC_COUNTER, "# smap delete miss"           )\
    ACTION( smap_delete_ex,         METRIC_COUNTER, "# smap delete exceptions"     )\
    ACTION( smap_len,               METRIC_COUNTER, "# smap length requests"       )\
    ACTION( smap_len_ok,            METRIC_COUNTER, "# smap length success"        )\
    ACTION( smap_len_notfound,      METRIC_COUNTER, "# smap length miss"           )\
    ACTION( smap_len_ex,            METRIC_COUNTER, "# smap len exceptions"        )\
    ACTION( smap_find,              METRIC_COUNTER, "# smap find requests"         )\
    ACTION( smap_find_ok,           METRIC_COUNTER, "# smap find success"          )\
    ACTION( smap_find_notfound,     METRIC_COUNTER, "# smap find miss"             )\
    ACTION( smap_find_notamember,   METRIC_COUNTER, "# smap find not present"      )\
    ACTION( smap_find_ex,           METRIC_COUNTER, "# smap find exceptions"       )\
    ACTION( smap_get,               METRIC_COUNTER, "# smap get requests"          )\
    ACTION( smap_get_ok,            METRIC_COUNTER, "# smap get success"           )\
    ACTION( smap_get_notfound,      METRIC_COUNTER, "# smap get miss"              )\
    ACTION( smap_get_oob,           METRIC_COUNTER, "# smap get out of bound"      )\
    ACTION( smap_get_ex,            METRIC_COUNTER, "# smap get exceptions"        )\
    ACTION( smap_insert,            METRIC_COUNTER, "# smap insert requests"       )\
    ACTION( smap_insert_ok,         METRIC_COUNTER, "# smap insert success"        )\
    ACTION( smap_insert_notfound,   METRIC_COUNTER, "# smap insert miss"           )\
    ACTION( smap_insert_noop,       METRIC_COUNTER, "# smap insert no action"      )\
    ACTION( smap_insert_trim,       METRIC_COUNTER, "# smap insert lead to trim"   )\
    ACTION( smap_insert_ex,         METRIC_COUNTER, "# smap insert exceptions"     )\
    ACTION( smap_remove,            METRIC_COUNTER, "# smap remove requests"       )\
    ACTION( smap_remove_ok,         METRIC_COUNTER, "# smap remove success"        )\
    ACTION( smap_remove_notfound,   METRIC_COUNTER, "# smap remove miss"           )\
    ACTION( smap_remove_noop,       METRIC_COUNTER, "# smap remove no-op"          )\
    ACTION( smap_remove_ex,         METRIC_COUNTER, "# smap remove exceptions"     )\
    ACTION( smap_truncate,          METRIC_COUNTER, "# smap truncate requests"     )\
    ACTION( smap_truncate_ok,       METRIC_COUNTER, "# smap truncate success"      )\
    ACTION( smap_truncate_notfound, METRIC_COUNTER, "# smap truncate miss"         )\
    ACTION( smap_truncate_ex,       METRIC_COUNTER, "# smap truncate exceptions"   )

struct request;
struct response;
struct command;

/* cmd_* functions must be command_fn (process.c) compatible */
void cmd_smap_create(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_delete(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_truncate(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_len(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_find(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_get(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_insert(struct response *rsp, const struct request *req, const struct command *cmd);
void cmd_smap_remove(struct response *rsp, const struct request *req, const struct command *cmd);

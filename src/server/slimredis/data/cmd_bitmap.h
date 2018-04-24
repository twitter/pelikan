#pragma once

/*          name                    type            description */
#define PROCESS_BITMAP_METRIC(ACTION)                                   \
    ACTION( bitmap_create,          METRIC_COUNTER, "# bitmap create requests" )\
    ACTION( bitmap_create_exist,    METRIC_COUNTER, "# bitmap already exist"   )\
    ACTION( bitmap_create_stored,   METRIC_COUNTER, "# bitmap stored"          )\
    ACTION( bitmap_create_ex,       METRIC_COUNTER, "# bitmap create exception")\
    ACTION( bitmap_delete,          METRIC_COUNTER, "# bitmap delete requests" )\
    ACTION( bitmap_delete_deleted,  METRIC_COUNTER, "# bitmap delete success"  )\
    ACTION( bitmap_delete_notfound, METRIC_COUNTER, "# bitmap delete notfound" )\
    ACTION( bitmap_get,             METRIC_COUNTER, "# bitmap get requests"    )\
    ACTION( bitmap_set,             METRIC_COUNTER, "# bitmap set requests"    )

struct request;
struct response;
struct command;

/* cmd_* functions must be command_fn (process.c) compatible */
void cmd_bitmap_create(struct response *rsp, struct request *req, struct command *cmd);
void cmd_bitmap_delete(struct response *rsp, struct request *req, struct command *cmd);
void cmd_bitmap_get(struct response *rsp, struct request *req, struct command *cmd);
void cmd_bitmap_set(struct response *rsp, struct request *req, struct command *cmd);

#pragma once

/*          name        type            description */
#define PROCESS_MISC_METRIC(ACTION)                             \
    ACTION( flushall,   METRIC_COUNTER, "# flushall requests"  )\
    ACTION( ping,       METRIC_COUNTER, "# ping requests"      )

struct request;
struct response;
struct command;

/* cmd_* functions must be command_fn (process.c) compatible */
void cmd_ping(struct response *rsp, struct request *req, struct command *cmd);

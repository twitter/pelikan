#pragma once

struct request;
struct response;

void admin_process_request(struct response *rsp, struct request *req);

#pragma once

struct request;
struct response;

/**
 * Responses can be chained, using the same field that supports pooling. It is
 * the responsibility of the caller to provide enough response structs if more
 * than one response is necessary- e.g. get/gets commands with batching, or
 * the stats command.
 *
 * Since response pool is not thread-safe, it is very important not trying to
 * use the same response pool from more than one thread, including the helper
 * thread(s). When the need arises for that, we will need to support resource
 * pool(s) that are either thread-local or identifiable instead of static ones.
 */
void process_request(struct response *rsp, struct request *req);

#pragma once

struct request;
struct response;
struct buf_sock;

void pubsub_setup(void);
void pubsub_teardown(void);

void command_subscribe(struct response *rsp, struct request *req, struct buf_sock *s);
void command_unsubscribe(struct response *rsp, struct request *req, struct buf_sock *s);
void command_publish(struct response *rsp, struct request *req, struct buf_sock *s);

#pragma once

struct op;
struct reply;

void process_op(struct reply *rep, struct op *op);

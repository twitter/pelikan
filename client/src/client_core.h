#pragma once

#include <cc_define.h>

#include <netdb.h>
#include <sys/socket.h>
#include <sys/types.h>

rstatus_i client_core_setup(struct addrinfo *ai);
void client_core_teardown(void);

void client_core_run(void);

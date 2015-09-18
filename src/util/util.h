#pragma once

#include <cc_define.h>

struct addrinfo;

/* Daemonize the process (have it run in the background) */
void daemonize(void);

/* Print the current version executable */
void show_version(void);

/* Init ai */
rstatus_t getaddr(struct addrinfo **ai, char *hostname, char *servname);

/* Create pid file */
void create_pidfile(const char *filename);

/* Remove pid file */
void remove_pidfile(const char *filename);

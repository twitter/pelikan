#include <util/bb_core_shared.h>

/* Needs to be initialized, or we run into linker errors */
int conn_fds[2] = {0, 0};

struct ring_array *conn_arr;

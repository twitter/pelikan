#include <core/data/shared.h>

#include <stdlib.h>             /* for NULL */

/* needs to be initialized to avoid linker issues due to being optimized out */
struct pipe_conn *pipe_c = NULL;

struct ring_array *conn_arr = NULL;

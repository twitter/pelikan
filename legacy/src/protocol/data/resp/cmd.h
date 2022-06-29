#pragma once

#include <stdint.h>

/* Common macros for defining a command */
#define CMD_OFFSET 1 /* first element in a request should be the command */

/* Allow unlimited optional parameters */
#define OPT_VARIED UINT32_MAX

#pragma once

//#include "seg.h"

#include <inttypes.h>
#include <stdbool.h>
//#include <stdio.h>
//#include <errno.h>
//#include <stdlib.h>
//#include <string.h>
//#include <sysexits.h>

//#include <cc_mm.h>

struct merge_opts {

    int32_t seg_n_merge;
    int32_t seg_n_max_merge;

    double  target_ratio;
    /* if the bytes on the merged seg is more than the threshold,
     * we stop merge process */
    double  stop_ratio;
    int32_t stop_bytes;

};

bool
check_merge_seg(void);
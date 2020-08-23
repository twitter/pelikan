#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>


#define N_BUCKET_PER_STEP_N_BIT     8u
#define N_BUCKET_PER_STEP           (1u << N_BUCKET_PER_STEP_N_BIT)

#define TTL_BUCKET_INTVL_N_BIT1     3u
#define TTL_BUCKET_INTVL_N_BIT2     7u
#define TTL_BUCKET_INTVL_N_BIT3     11u
#define TTL_BUCKET_INTVL_N_BIT4     15u
#define TTL_BUCKET_INTVL1           (1u << TTL_BUCKET_INTVL_N_BIT1)
#define TTL_BUCKET_INTVL2           (1u << TTL_BUCKET_INTVL_N_BIT2)
#define TTL_BUCKET_INTVL3           (1u << TTL_BUCKET_INTVL_N_BIT3)
#define TTL_BUCKET_INTVL4           (1u << TTL_BUCKET_INTVL_N_BIT4)

#define TTL_BOUNDARY1                                                          \
    (1u << (TTL_BUCKET_INTVL_N_BIT1 + N_BUCKET_PER_STEP_N_BIT))
#define TTL_BOUNDARY2                                                          \
    (1u << (TTL_BUCKET_INTVL_N_BIT2 + N_BUCKET_PER_STEP_N_BIT))
#define TTL_BOUNDARY3                                                          \
    (1u << (TTL_BUCKET_INTVL_N_BIT3 + N_BUCKET_PER_STEP_N_BIT))
#define TTL_BOUNDARY4                                                          \
    (1u << (TTL_BUCKET_INTVL_N_BIT4 + N_BUCKET_PER_STEP_N_BIT))




#define MAX_TTL                 (TTL_BOUNDARY4 - 1)
#define MAX_N_TTL_BUCKET        (N_BUCKET_PER_STEP * 4)
#define MAX_TTL_BUCKET_IDX      (MAX_N_TTL_BUCKET - 1)
#define ITEM_MAX_TTL            MAX_TTL

#define ITEM_MAGIC              ((uint32_t)0x0eedface)
#define SEG_MAGIC               ((uint32_t)0x0eadbeef)

#define SEG_HDR_SIZE            sizeof(struct seg)

#define ITEM_HDR_SIZE           offsetof(struct item, end)
#define ITEM_CAS_SIZE           (use_cas * sizeof(uint32_t))


#define TRACK_ADVANCED_STAT

#define USE_MERGE

#define SEG_MERGE_THRESHOLD     1
#define SEG_MERGE_AGE_LIMIT     0

#define N_RESERVED_SEG          1
#define N_MAX_SEG_MERGE         8

#define SEG_MERGE_STOP_RATIO 0.8
#define SEG_MERGE_MARGIN (int) (heap.seg_size * SEG_MERGE_STOP_RATIO)

#define N_SEG_MERGE             4
#define SEG_MERGE_TARGET_RATIO  ((1.0/N_SEG_MERGE)*1.05)

////#define ACTIVE_ITEM_START_REC_TIME  120
//#define ACTIVE_ITEM_START_REC_TIME  60
//#define ACTIVE_ITEM_START_REC_TIME  20
//#define ACTIVE_ITEM_START_REC_TIME  0
//#define ACTIVE_ITEM_START_REC_TIME  5


#pragma once

#include <stdbool.h>
#include <stdlib.h>
#include <stdint.h>


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

#define TTL_BOUNDARY1               \
    (2u << (TTL_BUCKET_INTVL_N_BIT1 + N_BUCKET_PER_STEP_N_BIT))
#define TTL_BOUNDARY2               \
    (2u << (TTL_BUCKET_INTVL_N_BIT2 + N_BUCKET_PER_STEP_N_BIT))
#define TTL_BOUNDARY3               \
    (2u << (TTL_BUCKET_INTVL_N_BIT3 + N_BUCKET_PER_STEP_N_BIT))
#define TTL_BOUNDARY4               \
    (2u << (TTL_BUCKET_INTVL_N_BIT4 + N_BUCKET_PER_STEP_N_BIT))


#ifdef do_not_defind
#define N_BUCKET_PER_STEP           256

#define TTL_BUCKET_INTVL1           8
#define TTL_BUCKET_INTVL2           128
#define TTL_BUCKET_INTVL3           2048
#define TTL_BUCKET_INTVL4           32576

#define TTL_BOUNDARY1              (TTL_BUCKET_INTVL1* N_BUCKET_PER_STEP)
#define TTL_BOUNDARY2              (TTL_BUCKET_INTVL2* N_BUCKET_PER_STEP)
#define TTL_BOUNDARY3              (TTL_BUCKET_INTVL3* N_BUCKET_PER_STEP)
#define TTL_BOUNDARY4              (TTL_BUCKET_INTVL4* N_BUCKET_PER_STEP)
#endif


#define MAX_TTL                     (TTL_BOUNDARY4 - 1)
#define MAX_TTL_BUCKET              (N_BUCKET_PER_STEP * 4)
#define MAX_TTL_BUCKET_IDX          (MAX_TTL_BUCKET - 1)
#define ITEM_MAX_TTL                MAX_TTL

#define ITEM_MAGIC                  ((uint32_t) 0x0eedface)
#define SEG_MAGIC                   ((uint32_t) 0x0eadbeef)

/* TODO(jason) consider making this an option */
#define LOCKTABLE_HASHPOWER         16u
#define HASHSIZE(n)       (1u << n)
#define HASHMASK(n)       (HASHSIZE(n) - 1u)

#define SEG_HDR_SIZE            sizeof(struct seg)

#define ITEM_HDR_SIZE           offsetof(struct item, end)
#define ITEM_CAS_SIZE           (use_cas * sizeof(uint32_t))



//#define CAS_TABLE_SIZE LOCKTABLE_SIZE
//#define CAS_HASHMASK    LOCKTABLE_HASHMASK


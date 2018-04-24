#pragma once

#include <stdint.h>

/* References:
 * https://github.com/RoaringBitmap/CRoaring/blob/master/include/roaring/bitset_util.h
 * https://stackoverflow.com/questions/47981/how-do-you-set-clear-and-toggle-a-single-bit
 */

/* fit in a 255-byte cuckoo cell with cuckoo header (6) and cas (8), bitset header (4) */
#define BITSET_COL_MAX (250 * 32)

/* NOTE: bitset must be allocated as 32-bit aligned */
struct bitset {
    uint8_t size;   /* in uint32_t => bitset can at most be 255*4 bytes */
    uint8_t col_w;  /* column width, defaults to 1 (bit), up to 8 (1 byte) */
    uint16_t count; /* non-zero column count */
    char data[1];   /* actual bitset data */
};

#define bit2byte(_col) ((((_col) - 1) >> 3) + 1)
#define bit2long(_col) ((((_col) - 1) >> 5) + 1)
#define size2bit(_sz) ((_sz) << 5)

void bitset_init(struct bitset *bs, uint16_t ncol);

uint8_t bitset_get(struct bitset *bs, uint16_t col);
/* Note: the interface is written as a generic set function with a val parameter
 * instead of two functions, set & clear, because we want to later support
 * multi-bit columns (up to a byte), so the values may go beyond 0 & 1
 */
void bitset_set(struct bitset *bs, uint16_t col, uint8_t val);

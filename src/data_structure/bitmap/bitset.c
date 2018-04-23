#include "bitset.h"

#include <stddef.h>


#define DATA_OFFSET(_bs) ((uint8_t *)(_bs) + offsetof(struct bitset, data))
#define SEGMENT_OFFSET(_col) ((uint8_t)(_col) >> 5) /* uint32_t, 2^5 bits per segment */
#define BIT_OFFSET(_col) ((uint8_t)(_col) & 0x1f)
#define GET_SEGMENT(_bs, _col)  \
        ((uint32_t *)DATA_OFFSET(_bs) + SEGMENT_OFFSET(_col))
#define GET_COL(_v, _offset) (((uint32_t)(_v) >> (_offset)) & 1UL)

void
bitset_init(struct bitset *bs, uint16_t ncol)
{
    int i;
    uint32_t *d = (uint32_t *)DATA_OFFSET(bs);

    bs->size = (uint8_t)bit2long(ncol);
    bs->col_w = 1;
    bs->count = 0;
    for (i = 0; i < bs->size; i++, d++) {
       *d = 0;
    }
}

uint8_t
bitset_get(struct bitset *bs, uint16_t col)
{
    return GET_COL(*GET_SEGMENT(bs, col), BIT_OFFSET(col));
}

void
bitset_set(struct bitset *bs, uint16_t col, uint8_t val)
{
    uint8_t offset = BIT_OFFSET(col);
    uint32_t *d = (uint32_t *)GET_SEGMENT(bs, col);

    bs->count += (val != 0) - (bitset_get(bs, col) != 0);

    /* clear column */
    *d &= ~(1UL << offset);
    /* set column */
    *d |= (uint32_t)val << offset;
}

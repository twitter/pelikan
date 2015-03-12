#ifndef _BB_ASSOC_H_
#define _BB_ASSOC_H_

/* This is a dummy header file included to allow other modules to compile. Actual
   hash facilities will be added at a later time. */

#include <stdint.h>

struct item;

static inline void assoc_insert(const struct item *it) {}
static inline void assoc_delete(const uint8_t *key, uint32_t nkey) {}
static inline struct item *assoc_find(const uint8_t *key, uint32_t nkey) { return NULL; }

#endif /* _BB_ASSOC_H_ */

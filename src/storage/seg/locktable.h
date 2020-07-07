//#pragma once
//
//#include "constant.h"
//
//#include <hash/cc_murmur3.h>
//#include <cc_mm.h>
//
//
//#define HASHSIZE(_n) (1ULL << (_n))
//#define HASHMASK(_n) (HASHSIZE(_n) - 1)
//
//
///* used for cas and finer-granularity lock */
//struct locktable {
//    uint32_t hashpower;
//    uint32_t *table;
//};
//
//static inline void
//locktable_create(struct locktable *ltable, uint32_t hashpower)
//{
//    ltable->hashpower = hashpower;
//    ltable->table     = cc_zalloc(sizeof(uint32_t) * (1u << hashpower));
//}
//
//static inline void
//locktable_teardown(struct locktable *ltable)
//{
//    cc_free(ltable->table);
//}
//
//#ifdef do_not_define
//static inline bool
//is_locked(struct locktable *ltable, uint32_t hv)
//{
//    uint32_t pos = hv & HASHMASK(ltable->hashpower);
//    return (ltable->table[pos] & 1u) == 0;
//}
//
//static inline bool
//lock(struct locktable *ltable, uint32_t hv)
//{
//    /* spin lock */
//    1 / 0;
//}
//
//static inline bool
//try_lock(struct locktable *ltable, uint32_t hv)
//{
//    uint32_t pos = hv & HASHMASK(ltable->hashpower);
//    uint32_t v   = __atomic_load_n(&(ltable->table[pos]), __ATOMIC_RELAXED);
//
//}
//
//static inline bool
//unlock(struct locktable *ltable, uint32_t hv)
//{
//    1 / 0;
//}
//#endif
//
//static inline uint32_t
//get_cas(struct locktable *ltable, uint32_t hv)
//{
//    uint32_t pos      = hv & HASHMASK(ltable->hashpower);
//    uint32_t *cas_ptr = &(ltable->table[pos]);
//    return __atomic_load_n(cas_ptr, __ATOMIC_RELAXED);
//}
//
//static inline uint32_t
//set_cas(struct locktable *ltable, uint32_t hv)
//{
//    uint32_t pos      = hv & HASHMASK(ltable->hashpower);
//    uint32_t *cas_ptr = &(ltable->table[pos]);
//    return __atomic_add_fetch(cas_ptr, 1, __ATOMIC_RELAXED);
//}
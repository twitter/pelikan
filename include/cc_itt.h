#pragma once

#include <cc_define.h>

#ifdef CC_ITT
#include "ittnotify.h"
#endif

#ifdef __cplusplus
extern "C" {
#endif
#ifdef CC_ITT

#define ITT_DOMAIN_NAME "cc_itt"

#define cc_declare_itt_function(_keyword, _name)                                 \
    _keyword __itt_heap_function _name

#define cc_create_itt_malloc(_name)                                              \
    _name = __itt_heap_function_create(#_name, ITT_DOMAIN_NAME)

#define cc_create_itt_free(_name)                                                \
    _name = __itt_heap_function_create(#_name, ITT_DOMAIN_NAME)

#define cc_create_itt_realloc(_name)                                             \
    _name = __itt_heap_function_create(#_name, ITT_DOMAIN_NAME)

#define cc_itt_alloc(_itt_heap_f, _p, _s) do {                                   \
    __itt_heap_allocate_begin(_itt_heap_f, (size_t)(_s), 0);                     \
    __itt_heap_allocate_end(_itt_heap_f, (void *)&(_p), (size_t)(_s), 0);        \
} while (0)

#define cc_itt_zalloc(_itt_heap_f, _p, _s) do {                                  \
    __itt_heap_allocate_begin(_itt_heap_f, (size_t)(_s), 1);                     \
    __itt_heap_allocate_end(_itt_heap_f, (void *)&(_p), (size_t)(_s), 1);        \
} while (0)

#define cc_itt_free(_itt_heap_f, _p) do {                                        \
    __itt_heap_free_begin(_itt_heap_f, _p);                                      \
    __itt_heap_free_end(_itt_heap_f, _p);                                        \
} while (0)

#define cc_itt_realloc(_itt_heap_f, _p, _np, _s) do {                            \
    __itt_heap_reallocate_begin(_itt_heap_f, _p, (size_t)(_s), 0);               \
    __itt_heap_reallocate_end(_itt_heap_f, _p, (void *)&(_np), (size_t)(_s), 0); \
} while (0)

#define cc_itt_heap_internal_access()                                            \
    __itt_heap_internal_access_begin()

#define cc_itt_heap_internal_access_end()                                        \
    __itt_heap_internal_access_end()

#else
#define cc_declare_itt_function(_keyword, _name)
#define cc_create_itt_malloc(_name)
#define cc_create_itt_free(_name)
#define cc_create_itt_realloc(_name)
#define cc_itt_alloc(_itt_heap_f, _p, _s)
#define cc_itt_zalloc(_itt_heap_f, _p, _s)
#define cc_itt_free(_itt_heap_f, _p)
#define cc_itt_realloc(_itt_heap_f, _p, _np, _s)
#define cc_itt_heap_internal_access_begin()
#define cc_itt_heap_internal_access_end()
#endif /* CC_ITT */

#ifdef __cplusplus
}
#endif

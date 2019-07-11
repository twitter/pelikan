/*
 * ccommon - a cache common library.
 * Copyright (C) 2013 Twitter, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include <cc_mm.h>

#include <cc_debug.h>

#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>

#ifdef OS_DARWIN
#   define MAP_ANONYMOUS MAP_ANON
#include <malloc/malloc.h>
#define malloc_usable_size malloc_size
#else
#include <malloc.h>
#endif

void *
_cc_alloc(size_t size, const char *name, int line)
{
    void *p;

    if (size == 0) {
        log_debug("malloc(0) @ %s:%d", name, line);
        return NULL;
    }

    p = malloc(size);
    if (p == NULL) {
        log_error("malloc(%zu) failed @ %s:%d", size, name, line);
    } else {
        log_vverb("malloc(%zu) at %p @ %s:%d", size, p, name, line);
    }

    return p;
}

void *
_cc_zalloc(size_t size, const char *name, int line)
{
    void *p;

    p = _cc_alloc(size, name, line);
    if (p != NULL) {
        memset(p, 0, size);
    }

    return p;
}

void *
_cc_calloc(size_t nmemb, size_t size, const char *name, int line)
{
    return _cc_zalloc(nmemb * size, name, line);
}

void *
_cc_realloc(void *ptr, size_t size, const char *name, int line)
{
    void *p;

    if (size == 0) {
        free(ptr);
        log_debug("realloc(0) @ %s:%d", name, line);
        return NULL;
    }

    p = realloc(ptr, size);
    if (p == NULL) {
        log_error("realloc(%zu) failed @ %s:%d", size, name, line);
    } else {
        log_vverb("realloc(%zu) at %p @ %s:%d", size, p, name, line);
    }

    return p;
}

void *
_cc_realloc_move(void *ptr, size_t size, const char *name, int line)
{
    void *p = NULL, *pr;

    if (size == 0) {
        free(ptr);
        log_debug("realloc(0) @ %s:%d", name, line);
        return NULL;
    }

    /*
     * Calling realloc then malloc allows us to force this function call to
     * change the address of the allocated memory block. realloc ensures we can
     * copy size bytes, and calling malloc before the realloc'd data is free'd
     * gives us a new address for the memory object.
     */
    if (((pr = realloc(ptr, size)) == NULL || (p = malloc(size)) == NULL)) {
        log_error("realloc(%zu) failed @ %s:%d", size, name, line);
    } else {
        log_vverb("realloc(%zu) at %p @ %s:%d", size, p, name, line);
        memcpy(p, pr, size);
    }

    free(pr);
    return p;
}

void
_cc_free(void *ptr, const char *name, int line)
{
    log_vverb("free(%p) @ %s:%d", ptr, name, line);
    free(ptr);
}

void *
_cc_mmap(size_t size, const char *name, int line)
{
    void *p;

    ASSERT(size != 0);

    /*
     * On success, mmap() returns a pointer to the mapped area.  On error,
     * the value MAP_FAILED (that is, (void *) -1) is returned, and errno
     * is set appropriately.
     */
    p = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS,
            -1, 0);
    if (p == ((void *) -1)) {
        log_error("mmap %zu bytes @ %s:%d failed: %s", size, name, line,
                strerror(errno));
        return NULL;
    }

    return p;
}

int
_cc_munmap(void *p, size_t size, const char *name, int line)
{
    int status;

    ASSERT(p != NULL);
    ASSERT(size != 0);

    /*
     * On success, munmap() returns 0, on failure -1, and errno is set
     * (probably to EINVAL).
     */
    status = munmap(p, size);
    if (status < 0) {
        log_error("munmap %p @ %s:%d failed: %s", p, name, line,
                strerror(errno));
    }

    return status;
}

size_t
_cc_alloc_usable_size(void *ptr, const char *name, int line)
{
    log_vverb("malloc_usable_size(%p) @ %s:%d", ptr, name, line);
    return malloc_usable_size(ptr);
}

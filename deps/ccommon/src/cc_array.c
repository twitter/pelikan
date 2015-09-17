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

#include <cc_array.h>

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mm.h>

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#define ARRAY_MODULE_NAME "ccommon::array"

static bool array_init = false;
static uint32_t max_nelem_delta = NELEM_DELTA;

/**
 * set up array and allocate data buffer in array
 * returns status of execution, error if out of memory
 */
rstatus_t
array_data_create(struct array *arr, uint32_t nalloc, size_t size)
{
    ASSERT(nalloc != 0 && size != 0);

    arr->data = cc_alloc(nalloc * size);
    if (arr->data == NULL) {
        log_info("array data creation failed due to OOM");

        return CC_ENOMEM;
    }

    arr->nelem = 0;
    arr->size = size;
    arr->nalloc = nalloc;

    return CC_OK;
}

/* free data buffer in array */
void
array_data_destroy(struct array *arr)
{
    if (arr->data != NULL) {
        cc_free(arr->data);
    }
}

/**
 * allocate an array and its data buffer
 * returns status of execution, error if out of memory
 */
rstatus_t
array_create(struct array **arr, uint32_t nalloc, size_t size)
{
    rstatus_t ret;

    ASSERT(nalloc != 0 && size != 0);

    *arr = (struct array *)cc_alloc(sizeof(**arr));
    if (arr == NULL) {
        log_info("array creation failed due to OOM");

        return CC_ENOMEM;
    }

    ret = array_data_create(*arr, nalloc, size);
    if (ret != CC_OK) {
        cc_free(*arr);
        return ret;
    }

    return CC_OK;
}

/**
 * free an array and its data buffer
 * require the address of array as argument to avoid dangling pointer
 */
void
array_destroy(struct array **arr)
{
    if (*arr == NULL) {
        return;
    }

    array_data_destroy(*arr);
    cc_free(*arr);
    *arr = NULL;
}

/*
 * expands the array by:
 * 1) doubling, if nelem is less than max_nelem_delta;
 * 2) adding max_nenem_delta elements
 */
static rstatus_t
_array_expand(struct array *arr)
{
    void *data;
    uint32_t nelem;
    size_t nbyte;

    nelem = arr->nalloc;
    if (arr->nalloc >= max_nelem_delta) {
        nelem += max_nelem_delta;
    } else {
        nelem *= 2;
    }
    nbyte = nelem * arr->size;
    data = cc_realloc(arr->data, nbyte);
    if (data == NULL) {
        return CC_ERROR;
    }

    arr->data = data;
    arr->nalloc = nelem;
    return CC_OK;
}

/*
 * push an element to array, returns the position of the new element.
 * Note: since the content is not passed in, the caller will be responsible for
 * filling out the data after the function returns with the position to write.
 */
void *
array_push(struct array *arr)
{
    rstatus_t status;

    if (arr->nelem == arr->nalloc) {
        /* the array is full; expand the data buffer */
        status = _array_expand(arr);
        if (status != CC_OK) {
            return NULL;
        }
    }

    arr->nelem++;

    return array_last(arr);
}

/* pop the last element */
void *
array_pop(struct array *arr)
{
    void *elem;

    elem = array_get(arr, arr->nelem - 1);
    arr->nelem--;

    return elem;
}

/* sort array data in ascending order based on the compare comparator */
void
array_sort(struct array *arr, array_compare_t compare)
{
    ASSERT(arr->nelem != 0);

    qsort(arr->data, arr->nelem, arr->size, compare);
}

/*
 * calls func with arg for each element in the array as long as func returns
 * success. On failure short-circuits, sets the error code and returns the idx
 * of element where the failure occurred
 */
uint32_t
array_each(struct array *arr, array_each_t func, void *arg, err_t *err)
{
    uint32_t i, nelem;

    ASSERT(arr->nelem != 0);
    ASSERT(func != NULL);

    for (i = 0, nelem = arr->nelem; i < nelem; i++) {
        void *elem = array_get(arr, i);
        rstatus_t status;

        status = func(elem, arg);
        if (status != CC_OK) {
            *err = status;
            return i;
        }
    }

    return nelem;
}

/* set the maximum number of elements allocated every time array expands */
void array_setup(uint32_t nelem)
{
    log_info("set up the %s module", ARRAY_MODULE_NAME);

    max_nelem_delta = nelem;
    if (array_init) {
        log_warn("%s has already been setup, overwrite", ARRAY_MODULE_NAME);
    }
    array_init = true;
}

void array_teardown(void)
{
    log_info("tear down the %s module", ARRAY_MODULE_NAME);
    if (!array_init) {
        log_warn("%s has never been setup", ARRAY_MODULE_NAME);
    }
    array_init = false;
}

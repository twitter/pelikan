#pragma once

#include <stddef.h>
#include <stdbool.h>
struct datapool;

struct datapool *datapool_open(const char *path, const char *user_signature,
    size_t size, int *fresh, bool prefault);
void datapool_close(struct datapool *pool);

void *datapool_addr(struct datapool *pool);
size_t datapool_size(struct datapool *pool);
void datapool_set_user_data(const struct datapool *pool, const void *user_data, size_t user_size);
void datapool_get_user_data(const struct datapool *pool, void *user_data, size_t user_size);

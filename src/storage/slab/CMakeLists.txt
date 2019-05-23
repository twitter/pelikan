set(SOURCE
    hashtable.c
    item.c
    slab.c)

add_library(slab ${SOURCE})
target_link_libraries(slab datapool)

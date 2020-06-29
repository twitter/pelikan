set(SOURCE
        hashtable.c
        item.c
        seg.c
        segevict.c
        ttlbucket.c)

add_library(seg ${SOURCE})
target_link_libraries(seg datapool)

set(SOURCE
        hashtable2.c
        item.c
        seg.c
        background.c
        segevict.c
        ttlbucket.c)

add_library(seg ${SOURCE})
target_link_libraries(seg datapool)

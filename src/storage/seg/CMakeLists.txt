set(SOURCE
        hashtable.c
        item.c
        seg.c
        background.c
        segevict.c
        segmerge.c
        ttlbucket.c)

add_library(seg ${SOURCE})
target_link_libraries(seg datapool)

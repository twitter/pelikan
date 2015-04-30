set (SOURCE
    bb_main.c
    bb_core.c
    bb_process.c
    )

set (LIBS
    slab
    memcache
    time
    ccommon-static
    )

add_executable(broadbill_twemcache ${SOURCE})
target_link_libraries (broadbill_twemcache ${LIBS})

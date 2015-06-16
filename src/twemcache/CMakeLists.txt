set (SOURCE
    main.c
    process.c
    )

set (LIBS
    slab
    memcache
    time
    util
    ccommon-static
    )

add_executable(broadbill_twemcache ${SOURCE})
target_link_libraries (broadbill_twemcache ${LIBS})

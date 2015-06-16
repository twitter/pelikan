set (SOURCE
    main.c
    process.c
    )

set (LIBS
    cuckoo
    memcache
    time
    util
    ccommon-static
    )

add_executable(broadbill_slimcache ${SOURCE})
target_link_libraries (broadbill_slimcache ${LIBS})

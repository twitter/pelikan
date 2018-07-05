if(TARGET_PINGSERVER)
    add_subdirectory(pingserver)
endif()

if(TARGET_REDIS)
    add_subdirectory(redis)
endif()

if(TARGET_SLIMREDIS)
    add_subdirectory(slimredis)
endif()

if(TARGET_SLIMCACHE)
    add_subdirectory(slimcache)
endif()

if(TARGET_TWEMCACHE)
    add_subdirectory(twemcache)
endif()

if(TARGET_CDB)
    add_subdirectory(cdb)
endif()

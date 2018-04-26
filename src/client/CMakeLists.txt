add_subdirectory(network)

if(TARGET_REDISCLI)
    add_subdirectory(redis_cli)
endif()

if(TARGET_MEMCACHECLI)
    add_subdirectory(memcache_cli)
endif()

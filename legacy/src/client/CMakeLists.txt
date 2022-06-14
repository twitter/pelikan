add_subdirectory(network)

if(TARGET_RESPCLI)
    add_subdirectory(resp_cli)
endif()

if(TARGET_MEMCACHECLI)
    add_subdirectory(memcache_cli)
endif()

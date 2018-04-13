if(TARGET_PINGSERVER)
    add_subdirectory(pingserver)
endif()

if(TARGET_PUBSUB)
    add_subdirectory(pubsub)
endif()

if(TARGET_SLIMCACHE)
    add_subdirectory(slimcache)
endif()

if(TARGET_TWEMCACHE)
    add_subdirectory(twemcache)
endif()

if(TARGET_PINGSERVER)
    add_subdirectory(pingserver)
endif()

if(TARGET_DS)
    add_subdirectory(ds)
endif()

if(TARGET_SLIMDS)
    add_subdirectory(slimds)
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

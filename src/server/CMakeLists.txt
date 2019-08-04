if(TARGET_PINGSERVER)
    add_subdirectory(pingserver)
endif()

if(TARGET_RDS)
    add_subdirectory(rds)
endif()

if(TARGET_SLIMRDS)
    add_subdirectory(slimrds)
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

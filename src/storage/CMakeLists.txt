if(TARGET_SLIMCACHE)
add_subdirectory(cuckoo)
endif()

if(TARGET_TWEMCACHE)
add_subdirectory(slab)
endif()

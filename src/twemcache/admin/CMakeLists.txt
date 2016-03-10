add_library(twemcache_admin process.c)
target_link_libraries(twemcache_admin protocol_admin ccommon-static)

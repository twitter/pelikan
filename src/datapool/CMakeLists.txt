add_library(datapool datapool.h)

if(USE_PMEM)
    target_sources(datapool PRIVATE datapool_pmem.c)
    target_include_directories(datapool PRIVATE ${LIBPMEM_INCLUDE_DIRS})
    target_link_libraries(datapool ${LIBPMEM_LIBRARIES})
else()
    target_sources(datapool PRIVATE datapool_shm.c)
endif(USE_PMEM)

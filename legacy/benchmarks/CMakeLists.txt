add_subdirectory(storage_cuckoo)
add_subdirectory(storage_slab)

set(SOURCE bench_storage.c)

set(MODULES_SLAB
    bench_storage_slab
    slab
    time)

set(MODULES_CUCKOO
    bench_storage_cuckoo
    cuckoo
    time)

set(LIBS
    ccommon-static
    ${CMAKE_THREAD_LIBS_INIT})

add_executable(bench_slab ${SOURCE})
target_link_libraries(bench_slab ${MODULES_SLAB} ${LIBS})

add_executable(bench_cuckoo ${SOURCE})
target_link_libraries(bench_cuckoo ${MODULES_CUCKOO} ${LIBS})

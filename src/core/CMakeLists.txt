add_subdirectory(admin)
add_subdirectory(server)
add_subdirectory(worker)

set(SOURCE
    core.c
    shared.c)

add_library(core ${SOURCE})
target_link_libraries(core core_admin core_server core_worker)

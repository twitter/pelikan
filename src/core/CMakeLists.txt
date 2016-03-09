add_subdirectory(admin)
add_subdirectory(data)

set(SOURCE
    core.c)

add_library(core ${SOURCE})
target_link_libraries(core core_admin core_data)

add_subdirectory(admin)
add_subdirectory(data)

set(SOURCE
    ${SOURCE}
    core.c)

add_library(core ${SOURCE})

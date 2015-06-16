set (SOURCE
    core.c
    core_server.c
    core_shared.c
    core_worker.c
    procinfo.c
    util.c
    )

add_library(util ${SOURCE})

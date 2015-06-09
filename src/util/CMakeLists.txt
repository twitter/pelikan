set (SOURCE
    bb_core.c
    bb_core_server.c
    bb_core_shared.c
    bb_core_worker.c
    bb_procinfo.c
    bb_util.c
    )

add_library(util ${SOURCE})

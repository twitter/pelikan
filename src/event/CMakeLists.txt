if(OS_PLATFORM STREQUAL "OS_DARWIN")
    set(SOURCE
        ${SOURCE}
        event/cc_shared.c
        event/cc_kqueue.c
        PARENT_SCOPE)
elseif(OS_PLATFORM STREQUAL "OS_LINUX")
    set(SOURCE
        ${SOURCE}
        event/cc_shared.c
        event/cc_epoll.c
        PARENT_SCOPE)
endif()

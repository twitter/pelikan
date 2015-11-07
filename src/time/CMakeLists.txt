if(OS_PLATFORM STREQUAL "OS_DARWIN")
    set(SOURCE
        ${SOURCE}
        time/cc_timer_darwin.c
        time/cc_wheel.c
        PARENT_SCOPE)
elseif(OS_PLATFORM STREQUAL "OS_LINUX")
    set(SOURCE
        ${SOURCE}
        time/cc_timer_linux.c
        time/cc_wheel.c
        PARENT_SCOPE)
endif()


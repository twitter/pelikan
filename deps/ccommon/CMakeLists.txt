cmake_minimum_required (VERSION 2.6)
project (ccommon)

# detect platform
set (OperatingSystem "OS_UNKNOWN")

if (${CMAKE_SYSTEM_NAME} MATCHES "Darwin")
    set (OperatingSystem "OS_DARWIN")
elseif (${CMAKE_SYSTEM_NAME} MATCHES "Linux")
    set (OperatingSystem "OS_LINUX")
endif ()

if (OperatingSystem STREQUAL "OS_UNKNOWN")
    message(FATAL_ERROR "unsupported operating system")
endif()

# the following sections work with config.h(.in): version, compile variables
# config.h.in has to include entries set/tested here for them to have effect

# version info
set (CCOMMON_VERSION_MAJOR 1)
set (CCOMMON_VERSION_MINOR 0)
set (CCOMMON_VERSION_PATCH 1)
set (CCOMMON_VERSION
    ${CCOMMON_VERSION_MAJOR}.${CCOMMON_VERSION_MINOR}.${CCOMMON_VERSION_PATCH}
    )
set (CCOMMON_RELEASE_VERSION
    ${CCOMMON_VERSION_MAJOR}.${CCOMMON_VERSION_MINOR}
    )

# flags => compile-time variables: use modules/macros
option (HAVE_ASSERT_LOG "assert_log enabled by default" ON)
option (HAVE_ASSERT_PANIC "assert_panic disabled by default" OFF)
option (HAVE_LOGGING "logging enabled by default" ON)
option (HAVE_STATS "stats enabled by default" ON)

include (CheckSymbolExists)
check_symbol_exists (sys_signame signal.h HAVE_SIGNAME)

include (CheckFunctionExists)
check_function_exists (backtrace HAVE_BACKTRACE)

include (TestBigEndian)
test_big_endian (HAVE_BIG_ENDIAN)

# how to use config.h.in to generate config.h
# this has to be set _after_ the above checks
configure_file (
  "${PROJECT_SOURCE_DIR}/config.h.in"
  "${PROJECT_BINARY_DIR}/config.h"
  )

# set compiler flags
# string concat is easier in 3.0, but older versions don't have the concat subcommand
# so we are using list as input until we move to new version
# TODO add build types
add_definitions (-D_GNU_SOURCE -D_FILE_OFFSET_BITS=64)
set (CFLAGS_LIST
    "-std=c11 "
    "-ggdb3 -O2 "
    "-Wall "
    "-Wmissing-prototypes -Wmissing-declarations -Wredundant-decls "
    "-Wunused-function -Wunused-value -Wunused-variable "
    "-fstrict-aliasing "
    )
string (REPLACE "" "" CFLAGS ${CFLAGS_LIST})
set (CMAKE_C_FLAGS
    "${CMAKE_C_FLAGS}  ${CFLAGS}"
    )

# where to find include files
include_directories ("${PROJECT_BINARY_DIR}" include)

# all source files listed here
set (SOURCE
    src/cc_array.c
    src/cc_bstring.c
    src/cc_debug.c
    src/cc_log.c
    src/cc_mm.c
    src/cc_option.c
    src/cc_print.c
    src/cc_rbuf.c
    src/cc_ring_array.c
    src/cc_signal.c
    src/buffer/cc_buf.c
    src/buffer/cc_dbuf.c
    src/channel/cc_pipe.c
    src/channel/cc_tcp.c
    src/event/cc_shared.c
    src/hash/cc_hash.c
    src/hash/cc_lookup3.c
    src/stats/cc_metric.c
    src/stream/cc_sockio.c
    )
# some files are platform-specific
if (OperatingSystem STREQUAL "OS_DARWIN")
    set (SOURCE
        ${SOURCE}
        src/event/cc_kqueue.c
        src/time/cc_timer_darwin.c
        )
elseif (OperatingSystem STREQUAL "OS_LINUX")
    set (SOURCE
        ${SOURCE}
        src/event/cc_epoll.c
        src/time/cc_timer_linux.c
        )
endif ()

# targets to build: here we have both static and dynmaic libs
set (LIBRARY_OUTPUT_PATH ${PROJECT_BINARY_DIR}/lib)
add_library (ccommon-static STATIC ${SOURCE})
add_library (ccommon-shared SHARED ${SOURCE})
set_target_properties (ccommon-static
    PROPERTIES
    OUTPUT_NAME ccommon-${CCOMMON_VERSION}
    )
set_target_properties (ccommon-shared
    PROPERTIES
    OUTPUT_NAME ccommon
    VERSION ${CCOMMON_VERSION}
    SOVERSION 0
    )

# install instructions
install (TARGETS ccommon-static DESTINATION lib)
install (TARGETS ccommon-shared DESTINATION lib)
install (DIRECTORY include/
    DESTINATION include/ccommon-${CCOMMON_RELEASE_VERSION}
    FILES_MATCHING PATTERN "*.h")

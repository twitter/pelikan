cmake_minimum_required (VERSION 2.6)
project (Broadbill)

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
set (Broadbill_VERSION_MAJOR 0)
set (Broadbill_VERSION_MINOR 1)
set (Broadbill_VERSION_PATCH 0)
set (Broadbill_VERSION
    ${Broadbill_VERSION_MAJOR}.${Broadbill_VERSION_MINOR}.${Broadbill_VERSION_PATCH}
    )
# flags => compile-time variables: use modules/macros
option (HAVE_ASSERT_LOG "assert_log enabled by default" ON)
option (HAVE_ASSERT_PANIC "assert_panic disabled by default" OFF)
option (HAVE_LOGGING "logging enabled by default" ON)
option (HAVE_STATS "stats enabled by default" ON)

include (CheckFunctionExists)
check_function_exists (backtrace HAVE_BACKTRACE)
check_function_exists (epoll_wait HAVE_EPOLL)
check_function_exists (kevent HAVE_KQUEUE)

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
# TODO once we add build types, we should also set flags such as "-O2 "
add_definitions (-D_GNU_SOURCE -D_FILE_OFFSET_BITS=64)
set (CFLAGS_LIST
    "-std=c11 "
    "-ggdb3 "
    "-Wall -Wshadow -Winline "
    "-Wstrict-prototypes -Wmissing-prototypes "
    "-Wmissing-declarations -Wredundant-decls "
    "-Wunused-function -Wunused-value -Wunused-variable "
    "-fno-strict-aliasing "
    )
string (REPLACE "" "" CFLAGS ${CFLAGS_LIST})
set (CMAKE_C_FLAGS
    "${CMAKE_C_FLAGS}  ${CFLAGS}"
    )

set (CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_HOME_DIRECTORY}/bin)
# build dependencies

# dependency: libccommon
set (CCOMMON_SOURCE_DIR "${PROJECT_SOURCE_DIR}/../ccommon" CACHE PATH "Path to the ccommon")
add_subdirectory (${CCOMMON_SOURCE_DIR} ${PROJECT_BINARY_DIR}/ccommon)

#set (CMAKE_INCLUDE_CURRENT_DIR)
include_directories (${include_directories}
    "${PROJECT_BINARY_DIR}"
    "${CCOMMON_SOURCE_DIR}/include"
    "${PROJECT_SOURCE_DIR}/src"
    )

# build executables
add_subdirectory (src)

# print a summary
message (STATUS "CPPFLAGS:" ${CMAKE_CPP_FLAGS})
message (STATUS "CFLAGS:" ${CMAKE_C_FLAGS})

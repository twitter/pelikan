cmake_minimum_required(VERSION 2.8)
project(pelikan C)

enable_testing()

# detect platform
macro(set_platform system_name)
    if(${system_name} MATCHES "Darwin")
        set(OS_PLATFORM "OS_DARWIN")
    elseif(${system_name} MATCHES "Linux")
        set(OS_PLATFORM "OS_LINUX")
    else()
        set(OS_PLATFORM "OS_UNSUPPORTED")
    endif()
endmacro(set_platform)

set_platform(${CMAKE_SYSTEM_NAME})
if(OS_PLATFORM STREQUAL "OS_UNSUPPORTED")
    message(FATAL_ERROR "unsupported operating system")
endif()

# the following sections work with config.h(.in): version, compile variables
# config.h.in has to include entries set/tested here for them to have effect

# version info
set(${PROJECT_NAME}_VERSION_MAJOR 0)
set(${PROJECT_NAME}_VERSION_MINOR 1)
set(${PROJECT_NAME}_VERSION_PATCH 1)
set(${PROJECT_NAME}_VERSION ${${PROJECT_NAME}_VERSION_MAJOR}.${${PROJECT_NAME}_VERSION_MINOR}.${${PROJECT_NAME}_VERSION_PATCH})

# flags => compile-time variables: use modules/macros
option(HAVE_ASSERT_LOG "assert_log enabled by default" ON)
option(HAVE_ASSERT_PANIC "assert_panic disabled by default" OFF)
option(HAVE_LOGGING "logging enabled by default" ON)
option(HAVE_STATS "stats enabled by default" ON)
option(TARGET_PINGSERVER "build pingserver binary" ON)
option(TARGET_SLIMCACHE "build slimcache binary" ON)
option(TARGET_TWEMCACHE "build twemcache binary" ON)
option(COVERAGE "code coverage" OFF)

# Note: duplicate custom targets only works with Makefile generators, will break XCode & VS
# reference: http://public.kitware.com/Bug/view.php?id=6348
set_property(GLOBAL PROPERTY ALLOW_DUPLICATE_CUSTOM_TARGETS 1)
set(CMAKE_SKIP_INSTALL_ALL_DEPENDENCY true)
set(CMAKE_MACOSX_RPATH 1)

include(CheckSymbolExists)
check_symbol_exists(sys_signame signal.h HAVE_SIGNAME)

include(CheckFunctionExists)
check_function_exists(backtrace HAVE_BACKTRACE)

include(TestBigEndian)
test_big_endian(HAVE_BIG_ENDIAN)

# how to use config.h.in to generate config.h
# this has to be set _after_ the above checks
configure_file(
  "${PROJECT_SOURCE_DIR}/cmake/config.h.in"
  "${PROJECT_BINARY_DIR}/config.h")

# set compiler flags
# string concat is easier in 3.0, but older versions don't have the concat subcommand
# so we are using list as input until we move to new version
# TODO once we add build types, we should also set flags such as "-O2 "
add_definitions(-D_GNU_SOURCE -D_FILE_OFFSET_BITS=64)
add_definitions()
set(CFLAGS_LIST
    "-std=c11 "
    "-ggdb3 -O2 "
    "-Wall -Wshadow -Winline "
    "-Wstrict-prototypes -Wmissing-prototypes "
    "-Wmissing-declarations -Wredundant-decls "
    "-Wunused-function -Wunused-value -Wunused-variable "
    "-fno-strict-aliasing ")

if(${OS_PLATFORM} MATCHES "OS_LINUX")
  set(CFLAGS_LIST "${CFLAGS_LIST} -lrt")
endif()

string(REPLACE "" "" LOCAL_CFLAGS ${CFLAGS_LIST})
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS}  ${LOCAL_CFLAGS}")

if (COVERAGE)
  SET(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -O0 -Wall -W -fprofile-arcs -ftest-coverage")
endif(COVERAGE)

# build dependencies

# dependency: libccommon
set(CCOMMON_SOURCE_DIR "${PROJECT_SOURCE_DIR}/deps/ccommon" CACHE PATH "Path to the ccommon")
add_subdirectory(${CCOMMON_SOURCE_DIR} ${PROJECT_BINARY_DIR}/ccommon)
# other dependencies
include(FindPackageHandleStandardArgs)
set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${PROJECT_SOURCE_DIR}/cmake")
find_package(Check)
if(NOT CHECK_FOUND)
    message(WARNING "Check is required to build and run tests")
endif(NOT CHECK_FOUND)
if(CHECK_FOUND)
    check_symbol_exists(ck_assert_int_eq check.h CHECK_WORKING)
    if(NOT CHECK_WORKING)
        message(WARNING "Check version too old to build tests")
    endif(NOT CHECK_WORKING)
endif(CHECK_FOUND)

find_package(Threads)

#set(CMAKE_INCLUDE_CURRENT_DIR)
include_directories(${include_directories}
    "${PROJECT_BINARY_DIR}"
    "${CCOMMON_SOURCE_DIR}/include"
    "${PROJECT_SOURCE_DIR}/src")

# server
add_subdirectory(src)

# tests: always build last
if(CHECK_WORKING)
    include_directories(${include_directories} "${CHECK_INCLUDES}")
    add_subdirectory(test)
endif(CHECK_WORKING)

# print a summary
message(STATUS "PLATFORM: " ${OS_PLATFORM})

message(STATUS "CPPFLAGS: " ${CMAKE_CPP_FLAGS})
message(STATUS "CFLAGS: " ${CMAKE_C_FLAGS})

message(STATUS "HAVE_SIGNAME: " ${HAVE_SIGNAME})

message(STATUS "HAVE_BACKTRACE: " ${HAVE_BACKTRACE})
message(STATUS "HAVE_BIG_ENDIAN: " ${HAVE_BIG_ENDIAN})

# Note: to uninstall targets, run:
#  xargs rm < install_manifest.txt

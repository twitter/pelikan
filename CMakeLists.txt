cmake_minimum_required(VERSION 2.6)
project(ccommon)

enable_testing()

###################
# detect platform #
###################

# TODO(yao):
#   1. make this a .cmake macro and put it under cmake/
#   2. avoid calling this twice when included by another project, e.g. Pelikan

macro(set_platform system_name)
    if(${system_name} MATCHES "Darwin")
        set(OS_PLATFORM "OS_DARWIN")
        add_definitions(-DOS_DARWIN)
    elseif(${system_name} MATCHES "Linux")
        set(OS_PLATFORM "OS_LINUX")
        add_definitions(-DOS_LINUX)
    else()
        set(OS_PLATFORM "OS_UNSUPPORTED")
    endif()
endmacro(set_platform)

set_platform(${CMAKE_SYSTEM_NAME})
if(OS_PLATFORM STREQUAL "OS_UNSUPPORTED")
    message(FATAL_ERROR "unsupported operating system")
endif()

####################
# define variables #
####################

# the following sections work with config.h(.in): version, compile variables
# config.h.in has to include entries set/tested here for them to have effect

# version info
set(${PROJECT_NAME}_VERSION_MAJOR 1)
set(${PROJECT_NAME}_VERSION_MINOR 0)
set(${PROJECT_NAME}_VERSION_PATCH 2)
set(${PROJECT_NAME}_VERSION
   ${${PROJECT_NAME}_VERSION_MAJOR}.${${PROJECT_NAME}_VERSION_MINOR}.${${PROJECT_NAME}_VERSION_PATCH}
   )
set(${PROJECT_NAME}_RELEASE_VERSION
   ${${PROJECT_NAME}_VERSION_MAJOR}.${${PROJECT_NAME}_VERSION_MINOR}
   )

# flags => compile-time variables: use modules/macros
option(HAVE_ASSERT_LOG "assert_log enabled by default" ON)
option(HAVE_ASSERT_PANIC "assert_panic disabled by default" OFF)
option(HAVE_LOGGING "logging enabled by default" ON)
option(HAVE_STATS "stats enabled by default" ON)
option(COVERAGE "code coverage" OFF)

include(CheckIncludeFiles)
if(OS_PLATFORM STREQUAL "OS_LINUX")
    check_include_files(linux/time64.h HAVE_TIME64)
endif()

include(CheckIncludeFiles)
if(OperatingSystem STREQUAL "OS_LINUX")
    check_include_files(linux/time64.h HAVE_TIME64)
endif()

include(CheckSymbolExists)
check_symbol_exists(sys_signame signal.h HAVE_SIGNAME)

include(CheckFunctionExists)
check_function_exists(backtrace HAVE_BACKTRACE)

include(TestBigEndian)
test_big_endian(HAVE_BIG_ENDIAN)

# how to use config.h.in to generate config.h
# this has to be set _after_ the above checks
configure_file(
  "${PROJECT_SOURCE_DIR}/config.h.in"
  "${PROJECT_BINARY_DIR}/config.h")


##########################
# other compiler options #
##########################

# set compiler flags
# string concat is easier in 3.0, but older versions don't have the concat subcommand
# so we are using list as input until we move to new version
# TODO add build types
add_definitions(-D_GNU_SOURCE -D_FILE_OFFSET_BITS=64)
set(CMAKE_MACOSX_RPATH 1)
set(CFLAGS_LIST
    "-std=c11 "
    "-ggdb3 -O2 "
    "-Wall "
    "-Wmissing-prototypes -Wmissing-declarations -Wredundant-decls "
    "-Wunused-function -Wunused-value -Wunused-variable "
    "-fstrict-aliasing ")
string(REPLACE "" "" CFLAGS ${CFLAGS_LIST})
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS}  ${CFLAGS}")

if (COVERAGE)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -O0 -Wall -W -fprofile-arcs -ftest-coverage")
endif(COVERAGE)

# test dependencies
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

# where to find include files
include_directories(${include_directories}
    "${PROJECT_BINARY_DIR}"
    "include")

###################
# things to build #
###################

add_subdirectory(src)
if(CHECK_WORKING)
    include_directories(${include_directories} "${CHECK_INCLUDES}")
    add_subdirectory(test)
endif(CHECK_WORKING)

###################
# print a summary #
###################

message(STATUS "PLATFORM: " ${OS_PLATFORM})

message(STATUS "CPPFLAGS: " ${CMAKE_CPP_FLAGS})
message(STATUS "CFLAGS: " ${CMAKE_C_FLAGS})

message(STATUS "HAVE_SIGNAME: " ${HAVE_SIGNAME})

message(STATUS "HAVE_BACKTRACE: " ${HAVE_BACKTRACE})
message(STATUS "HAVE_BIG_ENDIAN: " ${HAVE_BIG_ENDIAN})

message(STATUS "CHECK_WORKING: " ${CHECK_WORKING})

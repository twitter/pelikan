cmake_minimum_required(VERSION 2.8)
project(pelikan C)

enable_testing()

# detect platform
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
option(TARGET_DS "build dataserver binary" ON)
option(TARGET_SLIMDS "build slim dataserver binary" ON)
option(TARGET_SLIMCACHE "build slimcache binary" ON)
option(TARGET_TWEMCACHE "build twemcache binary" ON)
option(TARGET_CDB "build cdb binary (implies HAVE_RUST)" OFF)
option(TARGET_RESPCLI "build resp-cli binary" ON)

option(HAVE_RUST "build features written in rust" OFF)
option(RUST_USE_MUSL "build rust deps against musl" OFF)
option(BUILD_AND_INSTALL_CHECK "build our own version of check and link against it" OFF)
option(USE_PMEM "build persistent memory features" OFF)

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

if(BUILD_AND_INSTALL_CHECK)
    # (simms) What follows is a crime against build systems as we run the build/install
    # for the check library up front, during the planning phase.

    set(LIBCHECK_PREFIX "${CMAKE_BINARY_DIR}/check")

    # check for a local install of check
    if(NOT EXISTS "${LIBCHECK_PREFIX}")
        # (simms) This is terrible and I did it this way to ensure this gets built
        # before the rest of the 'check' tests run. This should be rewritten so that
        # the other dependencies know that there's a target that can build check
        execute_process(
            COMMAND "bash" "${PROJECT_SOURCE_DIR}/ci/install-check.sh" "${LIBCHECK_PREFIX}"
            TIMEOUT 300  # if this doesn't build in 5 minutes something is hosed
        )
    endif()

    set(CHECK_ROOT_DIR "${LIBCHECK_PREFIX}")
    set(CMAKE_REQUIRED_INCLUDES "${CHECK_ROOT_DIR}/include")    # these make check link correctly in ccommon and pelikan
    set(CMAKE_REQUIRED_LIBRARIES "${CHECK_ROOT_DIR}/lib")
endif()

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

find_package(PkgConfig QUIET)

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

if (USE_PMEM)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(LIBPMEM REQUIRED libpmem>=1.0)
    else()
        find_package(LIBPMEM REQUIRED 1.0)
    endif()
    link_directories(${LIBPMEM_LIBRARY_DIRS})
endif(USE_PMEM)

find_package(Threads)

if(TARGET_CDB)
    set(HAVE_RUST on)
    enable_language(Rust)
    include(CMakeCargo)
endif(TARGET_CDB)

#set(CMAKE_INCLUDE_CURRENT_DIR)
include_directories(${include_directories}
    "${PROJECT_BINARY_DIR}"
    "${CCOMMON_SOURCE_DIR}/include"
    "${PROJECT_SOURCE_DIR}/src")

# server & (cli) client
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

if(DUMP_ALL)
    message(STATUS "<<++=====------------------\\/------------------=====++>>")
    get_cmake_property(_variableNames VARIABLES)
    foreach (_variableName ${_variableNames})
        message(STATUS "${_variableName}=${${_variableName}}")
    endforeach()
    message(STATUS "<<++=====------------------/\\------------------=====++>>")
endif()

# Note: to uninstall targets, run:
#  xargs rm < install_manifest.txt
# vim:ts=4:sw=4:et

cmake_minimum_required(VERSION 3.0)
project(pelikan C)

# Uncomment the following to output dependency graph debugging messages
# set_property(GLOBAL PROPERTY GLOBAL_DEPENDS_DEBUG_MODE 1)

###################
# detect platform #
###################

# TODO(yao):
#   1. make this a .cmake macro and put it under cmake/
#   2. avoid calling this twice when included by another project, e.g. Pelikan

include(CheckSymbolExists)
# detect platform
macro(set_platform system_name)
    if(${system_name} MATCHES "Darwin")
        set(OS_PLATFORM "OS_DARWIN")
        add_definitions(-DOS_DARWIN)
    elseif(${system_name} MATCHES "Linux")
        set(OS_PLATFORM "OS_LINUX")
        check_symbol_exists(EFD_NONBLOCK "sys/eventfd.h" USE_EVENT_FD)
        add_definitions(-DOS_LINUX)
        if(USE_EVENT_FD)
            add_definitions(-DUSE_EVENT_FD)
        endif()
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
set(${PROJECT_NAME}_VERSION_MAJOR 0)
set(${PROJECT_NAME}_VERSION_MINOR 1)
set(${PROJECT_NAME}_VERSION_PATCH 1)
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
option(HAVE_TEST "test built by default" ON)
option(HAVE_DEBUG_MM "debugging oriented memory management disabled by default" OFF)
option(HAVE_COVERAGE "code coverage" OFF)
option(HAVE_RUST "rust bindings not built by default" OFF)
option(HAVE_ITT_INSTRUMENTATION "instrument code with ITT API" OFF)

option(FORCE_CHECK_BUILD "Force building check with ci/install-check.sh" OFF)

if(HAVE_RUST)
    option(RUST_VERBOSE_BUILD "pass -vv to cargo compilation" OFF)
endif()

option(TARGET_PINGSERVER "build pingserver binary" ON)
option(TARGET_RDS "build rich data server binary" ON)
option(TARGET_SLIMRDS "build slim rich data server binary" ON)
option(TARGET_SLIMCACHE "build slimcache binary" ON)
option(TARGET_TWEMCACHE "build twemcache binary" ON)
option(TARGET_CDB "build cdb binary (implies HAVE_RUST)" OFF)
option(TARGET_RESPCLI "build resp-cli binary" ON)
option(TARGET_HTTP "build experimental twemcache-http server (implies HAVE_RUST)" OFF)

option(USE_PMEM "build persistent memory features" OFF)


include(CheckFunctionExists)
check_function_exists(backtrace HAVE_BACKTRACE)

# how to use config.h.in to generate config.h
# this has to be set _after_ the above checks
configure_file(
  "${PROJECT_SOURCE_DIR}/cmake/config.h.in"
  "${PROJECT_BINARY_DIR}/config.h")

# Note: duplicate custom targets only works with Makefile generators, will break XCode & VS
# reference: http://public.kitware.com/Bug/view.php?id=6348
set_property(GLOBAL PROPERTY ALLOW_DUPLICATE_CUSTOM_TARGETS 1)
set(CMAKE_SKIP_INSTALL_ALL_DEPENDENCY true)
set(CMAKE_MACOSX_RPATH 1)

# Required for properly linking with rust code
set(CMAKE_POSITION_INDEPENDENT_CODE true)

# set compiler flags
# string concat is easier in 3.0, but older versions don't have the concat subcommand
# so we are using list as input until we move to new version
add_definitions(-D_GNU_SOURCE -D_FILE_OFFSET_BITS=64)
# Set a default build type (Release) if none was specified

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

if(CMAKE_BUILD_TYPE MATCHES Debug)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -O0")
else()
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -O2")
endif()
add_definitions()
set(CFLAGS_LIST
    "-std=c11 "
    "-ggdb3 "
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
endif()


if(${OS_PLATFORM} MATCHES "OS_LINUX")
  set(CFLAGS_LIST "${CFLAGS_LIST} -lrt")
endif()

string(REPLACE "" "" LOCAL_CFLAGS ${CFLAGS_LIST})
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS}  ${LOCAL_CFLAGS}")

if (COVERAGE)
    if(NOT ${CMAKE_BUILD_TYPE} MATCHES Debug)
        message(WARNING "Code coverage results with an optimised (non-Debug) build may be misleading" )
    endif()
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fprofile-arcs -ftest-coverage")
endif(COVERAGE)

# build dependencies

# dependency: libccommon
set(CCOMMON_SOURCE_DIR "${PROJECT_SOURCE_DIR}/deps/ccommon" CACHE PATH "Path to the ccommon")
add_subdirectory(${CCOMMON_SOURCE_DIR} ${PROJECT_BINARY_DIR}/ccommon)

# other dependencies
include(FindPackageHandleStandardArgs)
set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${PROJECT_SOURCE_DIR}/cmake")

# test dependencies
if (HAVE_TEST)
    enable_testing()
    # first we try default ways of finding gmodules
    find_package(CHECK)
    if(CHECK_FOUND)
        check_symbol_exists(ck_assert_int_eq check.h CHECK_WORKING)
    endif(CHECK_FOUND)
    # if we don't have a working version of check, build it
    if(NOT CHECK_WORKING OR FORCE_CHECK_BUILD)
        set(LIBCHECK_PREFIX "${CMAKE_BINARY_DIR}/check")
        execute_process(
            COMMAND "bash" "${PROJECT_SOURCE_DIR}/ci/install-check.sh" "${LIBCHECK_PREFIX}"
            TIMEOUT 300  # if this doesn't build in 5 minutes something is hosed
            RESULT_VARIABLE LIBCHECK_RETCODE
        )
        if(LIBCHECK_RETCODE) # non-zero means error
            message(STATUS "build libcheck failed, return code: " ${LIBCHECK_RETCODE})
        else(LIBCHECK_RETCODE)
            # use locally built libcheck
            set(CHECK_ROOT_DIR "${LIBCHECK_PREFIX}")
            find_package(CHECK)
        endif(LIBCHECK_RETCODE)
    endif(NOT CHECK_WORKING OR FORCE_CHECK_BUILD)

    # use fluxcapacitor to mock time
    if(OS_PLATFORM STREQUAL "OS_LINUX")
        set(FLUXCAP_PREFIX "${CMAKE_BINARY_DIR}/fluxcapacitor")
        execute_process(
            COMMAND "bash" "${PROJECT_SOURCE_DIR}/ci/install-fluxcapacitor.sh" "${FLUXCAP_PREFIX}"
            TIMEOUT 60  # if this doesn't build in 60 seconds something is hosed
            RESULT_VARIABLE FLUXCAP_RETCODE
        )
        if(FLUXCAP_RETCODE) # non-zero means error
            message(STATUS "build fluxcapacitor failed, return code: " ${FLUXCAP_RETCODE})
        else(FLUXCAP_RETCODE)
            set(FLUXCAP_BINARY "${FLUXCAP_PREFIX}/fluxcapacitor")
            message(STATUS "fluxcapacitor available at: " ${FLUXCAP_BINARY})
        endif(FLUXCAP_RETCODE)
    endif(OS_PLATFORM STREQUAL "OS_LINUX")
endif(HAVE_TEST)

find_package(PkgConfig QUIET)

if (USE_PMEM)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(LIBPMEM REQUIRED libpmem>=1.0)
    else()
        find_package(LIBPMEM REQUIRED 1.0)
    endif()
    link_directories(${LIBPMEM_LIBRARY_DIRS})
endif(USE_PMEM)

if (HAVE_ITT_INSTRUMENTATION)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(ITTNOTIFY REQUIRED ittnotify>=1.0)
    else()
        find_package(ITTNOTIFY REQUIRED 1.0)
    endif()
    include_directories(${ITTNOTIFY_INCLUDE_DIRS})
    link_directories(${ITTNOTIFY_LIBRARY_DIRS})
    link_libraries(${ITTNOTIFY_LIBRARIES})
endif(HAVE_ITT_INSTRUMENTATION)

find_package(Threads)

if(TARGET_CDB)
    set(HAVE_RUST on)
endif(TARGET_CDB)

if(TARGET_HTTP)
    set(HAVE_RUST on)
endif()

if(HAVE_RUST)
    include(cmake/CMakeCargo.cmake)
endif(HAVE_RUST)

#set(CMAKE_INCLUDE_CURRENT_DIR)
include_directories(${include_directories}
    "${PROJECT_BINARY_DIR}"
    "${CCOMMON_SOURCE_DIR}/include"
    "${PROJECT_SOURCE_DIR}/src"
    "${PROJECT_SOURCE_DIR}/benchmarks")

# server & (cli) client
add_subdirectory(src)

if(HAVE_TEST)
    include_directories(${include_directories} ${CHECK_INCLUDES})
    add_subdirectory(test)
    if(${OS_PLATFORM} MATCHES "OS_LINUX")
        add_subdirectory(benchmarks)
    endif()
endif(HAVE_TEST)


###################
# print a summary #
###################

message(STATUS "<<++=====------------------\\/------------------=====++>>")
message(STATUS "<<++                 pelikan summary                 ++>>")
message(STATUS "<<++=====------------------/\\------------------=====++>>")
message(STATUS "=============CMake related=============")
message(STATUS "CMAKE_BUILD_TYPE: " ${CMAKE_BUILD_TYPE})
message(STATUS "PLATFORM: " ${OS_PLATFORM})
message(STATUS "CPPFLAGS: " ${CMAKE_CPP_FLAGS})
message(STATUS "CFLAGS: " ${CMAKE_C_FLAGS})
message(STATUS "=======================================")

message(STATUS "=======Status of system features=======")
message(STATUS "HAVE_BACKTRACE: " ${HAVE_BACKTRACE})
message(STATUS "USE_EVENT_FD: " ${USE_EVENT_FD})
message(STATUS "=======================================")

message(STATUS "======Status of optional features======")
message(STATUS "HAVE_RUST: " ${HAVE_RUST})
message(STATUS "HAVE_ASSERT_LOG: " ${HAVE_ASSERT_LOG})
message(STATUS "HAVE_ASSERT_PANIC: " ${HAVE_ASSERT_PANIC})
message(STATUS "HAVE_LOGGING: " ${HAVE_LOGGING})
message(STATUS "HAVE_STATS: " ${HAVE_STATS})
message(STATUS "HAVE_ITT_INSTRUMENTATION: " ${HAVE_ITT_INSTRUMENTATION})
message(STATUS "HAVE_DEBUG_MM: " ${HAVE_DEBUG_MM})
message(STATUS "HAVE_TEST: " ${HAVE_TEST})
message(STATUS "HAVE_COVERAGE: " ${HAVE_COVERAGE})
message(STATUS "USE_PMEM: " ${USE_PMEM})
message(STATUS "=======================================")

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

if(HAVE_RUST)
    # Create a .cargo file which sets the target directory
    make_directory(.cargo)
    file(WRITE .cargo/config "[build]\ntarget-dir=\"${CMAKE_BINARY_DIR}/target\"\n")

    set_directory_properties(
        PROPERTIES
        ADDITIONAL_CLEAN_FILES "${CMAKE_BINARY_DIR}/target"
    )

    # Add a rust-docs target
    add_custom_target(
        rust-doc
        COMMAND ${CMAKE_COMMAND} -E env "CMAKE_BINARY_DIR=${CMAKE_BINARY_DIR}" "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/target" cargo doc
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
endif()

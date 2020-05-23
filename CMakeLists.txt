cmake_minimum_required(VERSION 3.0)
project(ccommon C)

# Uncomment the following to output dependency graph debugging messages
# set_property(GLOBAL PROPERTY GLOBAL_DEPENDS_DEBUG_MODE 1)

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
set(${PROJECT_NAME}_VERSION_MAJOR 2)
set(${PROJECT_NAME}_VERSION_MINOR 1)
set(${PROJECT_NAME}_VERSION_PATCH 0)
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

include(CheckIncludeFiles)
if(OS_PLATFORM STREQUAL "OS_LINUX")
    check_include_files(linux/time64.h HAVE_TIME64)
endif()

include(CheckSymbolExists)
check_symbol_exists(sys_signame signal.h HAVE_SIGNAME)

include(CheckFunctionExists)
check_function_exists(backtrace HAVE_BACKTRACE)
check_function_exists(accept4 HAVE_ACCEPT4)

# how to use config.h.in to generate config.h
# this has to be set _after_ the above checks
configure_file(
  "${PROJECT_SOURCE_DIR}/config.h.in"
  "${PROJECT_BINARY_DIR}/config.h")


##########################
# other compiler options #
##########################

# set compiler flags

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

set(CMAKE_MACOSX_RPATH 1)
string(CONCAT CFLAGS
    "-std=c11 "
    "-ggdb3 "
    "-Wall "
    "-Wmissing-prototypes -Wmissing-declarations -Wredundant-decls "
    "-Wunused-function -Wunused-value -Wunused-variable "
    "-fstrict-aliasing ")
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS}  ${CFLAGS}")

if(CMAKE_COMPILER_IS_GNUCC)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wl,--no-as-needed -ldl -pthread -fPIC")
endif()

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

if (HAVE_COVERAGE)
    if(NOT ${CMAKE_BUILD_TYPE} MATCHES Debug)
        message(WARNING "Code coverage results with an optimised (non-Debug) build may be misleading" )
    endif()
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fprofile-arcs -ftest-coverage")
endif(HAVE_COVERAGE)

# where to find include files
include_directories(
    ${include_directories}
    "${PROJECT_BINARY_DIR}"
    "include")

###################
# things to build #
###################

add_subdirectory(src)

if(HAVE_TEST)
    include_directories(${include_directories} ${CHECK_INCLUDES})
    add_subdirectory(test)
endif(HAVE_TEST)

if(HAVE_RUST)
    include(CMakeCargo)
    add_subdirectory(rust)

    if (${CMAKE_VERSION} VERSION_LESS "3.13.0")
        # CMakeCargo requires the use of some newer features of cmake
        # for changes to dependant libraries to cause a rebuild. This
        # will never break the build, it is just annoying for development.
        # Leave a warning here, but don't break the build for older cmake
        # versions.
        message(
            WARNING
            "Rust targets don't properly pick up changes to dependencies in cmake version <= 3.13"
        )
    endif()
endif()

###################
# print a summary #
###################

message(STATUS "<<++=====------------------\\/------------------=====++>>")
message(STATUS "<<++                 ccommon summary                 ++>>")
message(STATUS "<<++=====------------------/\\------------------=====++>>")
message(STATUS "=============CMake related=============")
message(STATUS "CMAKE_BUILD_TYPE: " ${CMAKE_BUILD_TYPE})
message(STATUS "PLATFORM: " ${OS_PLATFORM})
message(STATUS "CPPFLAGS: " ${CMAKE_CPP_FLAGS})
message(STATUS "CFLAGS: " ${CMAKE_C_FLAGS})
message(STATUS "=======================================")

message(STATUS "=======Status of system features=======")
message(STATUS "HAVE_SIGNAME: " ${HAVE_SIGNAME})
message(STATUS "HAVE_BACKTRACE: " ${HAVE_BACKTRACE})
message(STATUS "HAVE_ACCEPT4: " ${HAVE_ACCEPT4})
if(OS_PLATFORM STREQUAL "OS_LINUX")
    message(STATUS "HAVE_TIME64: " ${HAVE_TIME64})
endif()
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
message(STATUS "=======================================")


if(DUMP_ALL)
    message(STATUS "<<++=====------------------\\/------------------=====++>>")
    get_cmake_property(_variableNames VARIABLES)
    foreach (_variableName ${_variableNames})
        message(STATUS "${_variableName}=${${_variableName}}")
    endforeach()
    message(STATUS "<<++=====------------------/\\------------------=====++>>")
endif()

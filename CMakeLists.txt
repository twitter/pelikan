cmake_minimum_required(VERSION 2.8)
project(ccommon C)

# Uncomment the following to output dependency graph debugging messages
# set_property(GLOBAL PROPERTY GLOBAL_DEPENDS_DEBUG_MODE 1)

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
option(HAVE_DEBUG_MM "debugging oriented memory management disabled by default" OFF)
option(COVERAGE "code coverage" OFF)
option(HAVE_RUST "rust bindings not built by default" OFF)
option(HAVE_ITT_INSTRUMENTATION "instrument code with ITT API" OFF)

if(HAVE_RUST)
    option(RUST_VERBOSE_BUILD "pass -vv to cargo compilation" OFF)
endif()

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

set(CMAKE_MACOSX_RPATH 1)
set(CFLAGS_LIST
    "-std=c11 "
    "-ggdb3 "
    "-Wall "
    "-Wmissing-prototypes -Wmissing-declarations -Wredundant-decls "
    "-Wunused-function -Wunused-value -Wunused-variable "
    "-fstrict-aliasing ")
string(REPLACE "" "" CFLAGS ${CFLAGS_LIST})
set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS}  ${CFLAGS}")

if(CMAKE_COMPILER_IS_GNUCC)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wl,--no-as-needed -ldl -pthread -fPIC")
endif()

if (COVERAGE)
    if(NOT ${CMAKE_BUILD_TYPE} MATCHES Debug)
        message(WARNING "Code coverage results with an optimised (non-Debug) build may be misleading" )
    endif()
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fprofile-arcs -ftest-coverage")
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


# where to find include files
include_directories(
    ${include_directories}
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


if(HAVE_RUST)
    enable_language(Rust)
    include(CMakeCargo)
    add_subdirectory(rust)
    set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} -DHAVE_RUST=1")
endif()


###################
# print a summary #
###################

message(STATUS "CMAKE_BUILD_TYPE: " ${CMAKE_BUILD_TYPE})

message(STATUS "PLATFORM: " ${OS_PLATFORM})

message(STATUS "CPPFLAGS: " ${CMAKE_CPP_FLAGS})
message(STATUS "CFLAGS: " ${CMAKE_C_FLAGS})

message(STATUS "HAVE_SIGNAME: " ${HAVE_SIGNAME})

message(STATUS "HAVE_BACKTRACE: " ${HAVE_BACKTRACE})

message(STATUS "CHECK_WORKING: " ${CHECK_WORKING})

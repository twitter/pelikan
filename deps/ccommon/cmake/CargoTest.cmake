# ccommon - a cache common library.
# Copyright (C) 2019 Twitter, Inc.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Ensure that empty elements in lists aren't deleted
cmake_policy(SET CMP0007 NEW)

# Ignore the first 3 arguments since they will always be cmake -P <some path>/LinkRust.cmake
set(ARGI 3)
# This flips once we see -- in the arguments
set(PARSING_ENV_VARS OFF)
# Arguments to be passed directly to the build command
set(PASSTHROUGH_VARS )

# Split up the command-line arguments to this script
# into two groups
#
# Arguments before '--' are cmake variables that we set
# in this script. These are parameters which control the
# behaviour here.
#
# Arguments after '--' are environment variables to pass
# through to the cargo invocation, they are used by build
# scripts and to control cargo behaviour.
while(ARGI LESS ${CMAKE_ARGC})
    set(CURRENT_ARG ${CMAKE_ARGV${ARGI}})

    if(NOT PARSING_ENV_VARS)
        if(CURRENT_ARG STREQUAL "--")
            set(PARSING_ENV_VARS ON)
        else()
            string(REPLACE "=" ";" ARGLIST "${CURRENT_ARG}")

            list(GET ARGLIST 0 VAR)
            list(REMOVE_AT ARGLIST 0)
            string(REPLACE ";" "=" VALUE "${ARGLIST}")

            set(${VAR} "${VALUE}")
        endif()
    else()
        list(APPEND PASSTHROUGH_VARS "${CURRENT_ARG}")
    endif()

    math(EXPR ARGI "${ARGI} + 1")
endwhile()

file(
    READ
    "${LINK_FLAGS_FILE}"
    LINK_FLAGS
)

# This converts a space-delimited string to a cmake list
string(REPLACE " " ";" LINK_FLAGS_LIST "${LINK_FLAGS}")
set(LINK_FLAGS )

# To pass linker args through cargo we need to use
# the -Clink-arg=<flag> syntax.
foreach(FLAG ${LINK_FLAGS_LIST})
    if(EXISTS "${CMAKE_CURRENT_BINARY_DIR}/${FLAG}")
        get_filename_component(FLAG "${CMAKE_CURRENT_BINARY_DIR}/${FLAG}" ABSOLUTE)
    endif()

    list(APPEND LINK_FLAGS "-Clink-arg=${FLAG}")
endforeach()

string(REPLACE ";" " " LINK_FLAGS "${LINK_FLAGS}")
string(REPLACE " " ";" FLAGS "${FLAGS}")

# TODO(sean): We don't always want to colour the output. Is
#             there a way to autodetect this properly?
set(CARGO_COMMAND cargo test --color always ${FLAGS})

execute_process(
    COMMAND ${CMAKE_COMMAND} -E env ${PASSTHROUGH_VARS} "RUSTFLAGS=${LINK_FLAGS}" ${CARGO_COMMAND}
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    RESULT_VARIABLE STATUS
)

# Ensure that our script exits with the correct error code.
# The only way to get a cmake script to exit with an error
# code is to print a message so that's what we do here.
if(NOT STATUS EQUAL 0)
    # Dump some script state to help with debugging
    message(STATUS "PASSTHROUGH_VARS=${PASSTHROUGH_VARS}")

    message(FATAL_ERROR "cargo test failed")
endif()

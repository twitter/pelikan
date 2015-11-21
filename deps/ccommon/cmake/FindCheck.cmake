
#--------------------------------------------------------------------------------
# Copyright (c) 2013-2013, Lars Baehren <lbaehren@gmail.com>
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without modification,
# are permitted provided that the following conditions are met:
#
#  * Redistributions of source code must retain the above copyright notice, this
#    list of conditions and the following disclaimer.
#  * Redistributions in binary form must reproduce the above copyright notice,
#    this list of conditions and the following disclaimer in the documentation
#    and/or other materials provided with the distribution.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
# AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
# DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
# FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
# DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
# CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
# OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#--------------------------------------------------------------------------------

# Check is a unit testing framework for C. It features a simple interface for
# defining unit tests, putting little in the way of the developer. Tests are run
# in a separate address space, so Check can catch both assertion failures and
# code errors that cause segmentation faults or other signals. The output from
# unit tests can be used within source code editors and IDEs..
#
# The following variables are set when CHECK is found:
#  CHECK_FOUND      = Set to true, if all components of CHECK have been found.
#  CHECK_INCLUDES   = Include path for the header files of CHECK
#  CHECK_LIBRARIES  = Link these to use CHECK
#  CHECK_LFLAGS     = Linker flags (optional)

if (NOT CHECK_FOUND)

  if (NOT CHECK_ROOT_DIR)
    set (CHECK_ROOT_DIR ${CMAKE_INSTALL_PREFIX})
  endif (NOT CHECK_ROOT_DIR)

  ##_____________________________________________________________________________
  ## Check for the header files

  find_path (CHECK_INCLUDES
    NAMES check.h
    HINTS ${CHECK_ROOT_DIR} ${CMAKE_INSTALL_PREFIX}
    PATH_SUFFIXES include
    )

  ##_____________________________________________________________________________
  ## Check for the library

  find_library (CHECK_LIBRARIES check
    HINTS ${CHECK_ROOT_DIR} ${CMAKE_INSTALL_PREFIX}
    PATH_SUFFIXES lib
    )

  ##_____________________________________________________________________________
  ## Actions taken when all components have been found

  find_package_handle_standard_args (CHECK DEFAULT_MSG CHECK_LIBRARIES CHECK_INCLUDES)

  if (CHECK_FOUND)
    if (NOT CHECK_FIND_QUIETLY)
      message (STATUS "Found components for CHECK")
      message (STATUS "CHECK_ROOT_DIR  = ${CHECK_ROOT_DIR}")
      message (STATUS "CHECK_INCLUDES  = ${CHECK_INCLUDES}")
      message (STATUS "CHECK_LIBRARIES = ${CHECK_LIBRARIES}")
    endif (NOT CHECK_FIND_QUIETLY)
  else (CHECK_FOUND)
    if (CHECK_FIND_REQUIRED)
      message (FATAL_ERROR "Could not find CHECK!")
    endif (CHECK_FIND_REQUIRED)
  endif (CHECK_FOUND)

  ##_____________________________________________________________________________
  ## Mark advanced variables

  mark_as_advanced (
    CHECK_ROOT_DIR
    CHECK_INCLUDES
    CHECK_LIBRARIES
    )

endif (NOT CHECK_FOUND)

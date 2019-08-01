# ITTNOTIFY is the instrumentation and tracing technology (ITT) APIs provided by
# the Intel® VTune™Amplifier enable your application to generate and control
# the collection of trace data during its execution.
#
# The following variables are set when ITTNOTIFY is found:
#  ITTNOTIFY_FOUND          = Set to true, if all components of ITTNOTIFY have been found.
#  ITTNOTIFY_INCLUDE_DIRS   = Include path for the header files of ITTNOTIFY.
#  ITTNOTIFY_LIBRARY_DIRS   = Library search path for the ITTNOTIFY libraries.
#  ITTNOTIFY_LIBRARIES      = Link these to use ITTNOTIFY.
#  ITTNOTIFY_LFLAGS         = Linker flags (optional).

if (NOT ITTNOTIFY_FOUND)

  find_program(VTUNE_EXECUTABLE amplxe-cl)

  if(NOT VTUNE_EXECUTABLE)
    set(ITTNOTIFY_FOUND false)
	return()
  endif()

  get_filename_component(VTUNE_DIR ${VTUNE_EXECUTABLE} PATH)
  set(ITTNOTIFY_ROOT_DIR "${VTUNE_DIR}/..")

  ##_____________________________________________________________________________
  ## Check for the header files

  find_path (ITTNOTIFY_INCLUDE_DIRS
    NAMES ittnotify.h
    HINTS ${ITTNOTIFY_ROOT_DIR}
	PATHS /usr /usr/local
    PATH_SUFFIXES include
    )

  ##_____________________________________________________________________________
  ## Check for the library

  find_library (ITTNOTIFY_LIBRARIES ittnotify
    HINTS ${ITTNOTIFY_ROOT_DIR}
	PATHS /usr /usr/local /opt/local
    PATH_SUFFIXES lib64 lib
    )

  ##_____________________________________________________________________________
  ## Actions taken when all components have been found

  find_package_handle_standard_args (ITTNOTIFY DEFAULT_MSG ITTNOTIFY_LIBRARIES ITTNOTIFY_INCLUDE_DIRS)

  if (ITTNOTIFY_FOUND)
    if (NOT ITTNOTIFY_FIND_QUIETLY)
      message (STATUS "Found components for ITTNOTIFY")
      message (STATUS "ITTNOTIFY_ROOT_DIR  = ${ITTNOTIFY_ROOT_DIR}")
      message (STATUS "ITTNOTIFY_INCLUDE_DIRS  = ${ITTNOTIFY_INCLUDE_DIRS}")
      message (STATUS "ITTNOTIFY_LIBRARIES = ${ITTNOTIFY_LIBRARIES}")
    endif (NOT ITTNOTIFY_FIND_QUIETLY)
  else (ITTNOTIFY_FOUND)
    if (ITTNOTIFY_FIND_REQUIRED)
      message (FATAL_ERROR "Could not find ITTNOTIFY!")
    endif (ITTNOTIFY_FIND_REQUIRED)
  endif (ITTNOTIFY_FOUND)

  if(UNIX)
    list(APPEND ITTNOTIFY_LIBRARIES dl)
  endif()

endif (NOT ITTNOTIFY_FOUND)

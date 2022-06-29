# Find the libpmem library.
# Output variables:
#  LIBPMEM_INCLUDE_DIRS : e.g., /usr/include/.
#  LIBPMEM_LIBRARIES    : Library path of libpmem
#  LIBPMEM_FOUND        : True if found.

  ##_____________________________________________________________________________
  ## Check for the header files

  find_path (LIBPMEM_INCLUDE_DIRS
    NAMES libpmem.h
    PATH_SUFFIXES include
    )

  ##_____________________________________________________________________________
  ## Check for the library

  find_library (LIBPMEM_LIBRARIES pmem
    PATH_SUFFIXES lib64 lib
    )

  ##_____________________________________________________________________________
  ## Actions taken when all components have been found

  find_package_handle_standard_args (LIBPMEM DEFAULT_MSG LIBPMEM_LIBRARIES LIBPMEM_INCLUDE_DIRS)

if (LIBPMEM_FOUND)
    message (STATUS "Found components for LIBPMEM")
    message (STATUS "LIBPMEM_INCLUDE_DIRS  = ${LIBPMEM_INCLUDE_DIRS}")
    message (STATUS "LIBPMEM_LIBRARIES  = ${LIBPMEM_LIBRARIES}")
else ()
    message(FATAL_ERROR "Could not find LIBPMEM, download and install from https://github.com/pmem/pmdk/releases")
endif ()

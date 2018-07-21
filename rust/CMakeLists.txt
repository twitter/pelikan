file(WRITE CMAKE_BINARY_DIR "${CMAKE_BINARY_DIR}\n")

if(HAVE_RUST)
    add_subdirectory(ccommon_rs)
endif()

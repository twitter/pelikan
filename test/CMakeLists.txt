include_directories(${include_directories} CHECK_INCLUDES)

add_subdirectory(data_structure)
add_subdirectory(protocol)
add_subdirectory(hotkey)
add_subdirectory(storage)
add_subdirectory(time)
if(USE_PMEM)
    add_subdirectory(datapool)
endif()

add_subdirectory(integration)
add_subdirectory(server)

include_directories(${include_directories} CHECK_INCLUDES)

add_custom_target(check COMMAND ${CMAKE_CTEST_COMMAND})

add_subdirectory(data_structure)
add_subdirectory(protocol)
add_subdirectory(hotkey)
add_subdirectory(storage)

add_subdirectory(integration)

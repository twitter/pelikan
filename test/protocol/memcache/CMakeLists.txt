set(lib_name memcache)
set(test_name check_${lib_name})

set(source ${test_name}.c)

add_executable(${test_name} ${source})
target_link_libraries(${test_name} ${lib_name})
target_link_libraries(${test_name} ccommon-static ${CHECK_LIBRARIES})

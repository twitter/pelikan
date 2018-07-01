set(suite bitmap)
set(test_name check_${suite})

set(source check_${suite}.c)

add_executable(${test_name} ${source})
target_link_libraries(${test_name} ds_${suite})
target_link_libraries(${test_name} ccommon-static ${CHECK_LIBRARIES})
target_link_libraries(${test_name} pthread m)

add_dependencies(check ${test_name})
add_test(${test_name} ${test_name})

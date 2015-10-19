set(suite cuckoo)
set(test_name check_${suite})

set(source check_${suite}.c)

add_executable(${test_name} ${source})
target_link_libraries(${test_name} ${suite} time ccommon-static ${CHECK_LIBRARIES})

add_test(${test_name} ${test_name})

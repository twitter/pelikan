set(suite bstring)
set(test_name check_${suite})

set(source check_${suite}.c)

add_executable(${test_name} ${source})
target_link_libraries(${test_name} ccommon-static ${CHECK_LIBRARIES})
target_link_libraries(${test_name} m)
target_link_libraries(${test_name} pthread)
if (OS_PLATFORM STREQUAL "OS_LINUX")
  target_link_libraries(${test_name} rt)
endif(OS_PLATFORM STREQUAL "OS_LINUX")

add_test(${test_name} ${test_name})

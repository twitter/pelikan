set(suite log)
set(test_name check_${suite})

set(source check_${suite}.c)

set(CCOMMON_LIBS ccommon-static)

add_executable(${test_name} ${source})
target_link_libraries(
    ${test_name}
    ${CHECK_LIBRARIES}
    ${CMAKE_THREAD_LIBS_INIT}
    ${CCOMMON_LIBS}
    m)

if(OS_PLATFORM STREQUAL "OS_DARWIN")
    # the "rand" crate links to the platform secure rng API, so it requires this
    target_link_libraries(${test_name} "-framework Security")
    set_target_properties(${test_name} PROPERTIES LINK_FLAGS "-Wl,-F/System/Library/Frameworks")
endif()

add_test(${test_name} ${test_name})

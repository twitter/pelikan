set(suite log)
set(test_name check_${suite})

set(source check_${suite}.c)

if(HAVE_RUST)
    set(CCOMMON_LIBS ccommon_rs) # common_rs_static staticly links common-static
else()
    set(CCOMMON_LIBS ccommon-static)
endif()


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

add_dependencies(check ${test_name} ccommon-static)
add_test(${test_name} ${test_name})

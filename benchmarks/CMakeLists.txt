add_executable(bench_micro bench_micro.c)

target_include_directories(bench_micro PRIVATE
	${CMAKE_SOURCE_DIR}/src
	${CMAKE_SOURCE_DIR}/tests)

target_link_libraries(bench_micro cuckoo time)
target_link_libraries(bench_micro ccommon-static)
target_link_libraries(bench_micro pthread)

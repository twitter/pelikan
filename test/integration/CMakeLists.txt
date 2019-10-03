set(test_name integration)

include(FindPythonInterp)

set(ENV_COMMAND ${CMAKE_COMMAND} -E env "PELIKAN_BIN_PATH=${CMAKE_BINARY_DIR}/_bin")

add_test(NAME ${test_name}
         COMMAND ${ENV_COMMAND} ${PYTHON_EXECUTABLE} test_twemcache.py
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})

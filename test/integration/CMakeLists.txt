set(test_name integration)

include(FindPythonInterp)

add_test(NAME ${test_name}
         COMMAND ${PYTHON_EXECUTABLE} test_twemcache.py
         WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})

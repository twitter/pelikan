set(RUST_CDYLIB_BUILD yes)

cargo_build(NAME cdb_ffi)

message(STATUS "++========================================++")

get_cmake_property(_variableNames VARIABLES)
foreach (_variableName ${_variableNames})
    message(STATUS "${_variableName}=${${_variableName}}")
endforeach()

message(STATUS "++========================================++")


if(NOT CMAKE_Rust_COMPILER)
	find_package(Rust)
	if(RUST_FOUND)
		set(CMAKE_Rust_COMPILER "${RUSTC_EXECUTABLE}")
		set(CMAKE_Rust_COMPILER_ID "Rust")
		set(CMAKE_Rust_COMPILER_VERSION "${RUST_VERSION}")
		set(CMAKE_Rust_PLATFORM_ID "Rust")
	endif()
endif()

message(STATUS "Cargo Prefix: ${CARGO_PREFIX}")
message(STATUS "Rust Compiler Version: ${RUSTC_VERSION}")

mark_as_advanced(CMAKE_Rust_COMPILER)

if(CMAKE_Rust_COMPILER)
	set(CMAKE_Rust_COMPILER_LOADED 1)
endif(CMAKE_Rust_COMPILER)

configure_file(${CMAKE_SOURCE_DIR}/cmake/CMakeRustCompiler.cmake.in
	${CMAKE_BINARY_DIR}${CMAKE_FILES_DIRECTORY}/${CMAKE_VERSION}/CMakeRustCompiler.cmake IMMEDIATE @ONLY)

set(CMAKE_Rust_COMPILER_ENV_VAR "RUSTC")


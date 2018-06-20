function(cargo_set_lib_target LIB_NAME)
    if(WIN32)
        if(CMAKE_SIZEOF_VOID_P EQUAL 8)
            set(LIB_TARGET "x86_64-pc-windows-msvc" PARENT_SCOPE)
        else()
            set(LIB_TARGET "i686-pc-windows-msvc" PARENT_SCOPE)
        endif()
	elseif(ANDROID)
        if(ANDROID_SYSROOT_ABI STREQUAL "x86")
            set(LIB_TARGET "i686-linux-android" PARENT_SCOPE)
        elseif(ANDROID_SYSROOT_ABI STREQUAL "x86_64")
            set(LIB_TARGET "x86_64-linux-android" PARENT_SCOPE)
        elseif(ANDROID_SYSROOT_ABI STREQUAL "arm")
            set(LIB_TARGET "arm-linux-androideabi" PARENT_SCOPE)
        elseif(ANDROID_SYSROOT_ABI STREQUAL "arm64")
            set(LIB_TARGET "aarch64-linux-android" PARENT_SCOPE)
        endif()
    elseif(IOS)
		set(LIB_TARGET "universal" PARENT_SCOPE)
    elseif(CMAKE_SYSTEM_NAME STREQUAL Darwin)
        set(LIB_TARGET "x86_64-apple-darwin" PARENT_SCOPE)
	else()
        if(RUST_USE_MUSL)
            set(RUST_LIBC_NAME "musl")
        else()
            set(RUST_LIBC_NAME "gnu")
        endif()

        if(CMAKE_SIZEOF_VOID_P EQUAL 8)
            set(LIB_TARGET "x86_64-unknown-linux-${RUST_LIBC_NAME}" PARENT_SCOPE)
        else()
            set(LIB_TARGET "i686-unknown-linux-${RUST_LIBC_NAME}" PARENT_SCOPE)
        endif()
    endif()
endfunction()

function(cargo_build)
    cmake_parse_arguments(CARGO "" "NAME" "" ${ARGN})
    string(REPLACE "-" "_" LIB_NAME ${CARGO_NAME})

    get_filename_component(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}" ABSOLUTE)

    cargo_set_lib_target(LIB_NAME)

    if(NOT CMAKE_BUILD_TYPE)
        set(LIB_BUILD_TYPE "debug")
    elseif(${CMAKE_BUILD_TYPE} STREQUAL "Release")
        set(LIB_BUILD_TYPE "release")
    else()
        set(LIB_BUILD_TYPE "debug")
    endif()

    set(SHARED_LIB_FNAME "${CMAKE_SHARED_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_SHARED_LIBRARY_SUFFIX}")
    set(STATIC_LIB_FNAME "${CMAKE_STATIC_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_STATIC_LIBRARY_SUFFIX}")

    set(LIB_BASE_DIR "${CARGO_TARGET_DIR}/${LIB_TARGET}/${LIB_BUILD_TYPE}")

    get_filename_component(SHARED_LIB_FILE "${LIB_BASE_DIR}/${SHARED_LIB_FNAME}" ABSOLUTE)
    get_filename_component(STATIC_LIB_FILE "${LIB_BASE_DIR}/${STATIC_LIB_FNAME}" ABSOLUTE)

	if(IOS)
		set(CARGO_ARGS "lipo")
	else()
    	set(CARGO_ARGS "build")
		list(APPEND CARGO_ARGS "--target" ${LIB_TARGET})
	endif()

    if(${LIB_BUILD_TYPE} STREQUAL "release")
        list(APPEND CARGO_ARGS "--release")
    endif()

    file(GLOB_RECURSE LIB_SOURCES "*.rs")

    set(CARGO_ENV_COMMAND ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${CARGO_TARGET_DIR}")

    add_custom_command(
        OUTPUT ${STATIC_LIB_FILE}
        COMMAND ${CARGO_ENV_COMMAND} ${CARGO_EXECUTABLE} ARGS ${CARGO_ARGS}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
        DEPENDS ${LIB_SOURCES}
        COMMENT "running cargo")
    add_custom_target(${CARGO_NAME}_static_target ALL DEPENDS ${STATIC_LIB_FILE})
    add_library(${CARGO_NAME}_static STATIC IMPORTED GLOBAL)
    add_dependencies(${CARGO_NAME}_static ${CARGO_NAME}_static_target)
    set_target_properties(${CARGO_NAME}_static PROPERTIES IMPORTED_LOCATION ${STATIC_LIB_FILE})

    add_custom_command(
        OUTPUT ${SHARED_LIB_FILE}
        COMMAND ${CARGO_ENV_COMMAND} ${CARGO_EXECUTABLE} ARGS ${CARGO_ARGS}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
        DEPENDS ${LIB_SOURCES}
        COMMENT "running cargo")
    add_custom_target(${CARGO_NAME}_shared_target ALL DEPENDS ${SHARED_LIB_FILE})
    add_library(${CARGO_NAME}_shared SHARED IMPORTED GLOBAL)
    add_dependencies(${CARGO_NAME}_shared ${CARGO_NAME}_shared_target)
    set_target_properties(${CARGO_NAME}_shared PROPERTIES IMPORTED_LOCATION ${SHARED_LIB_FILE})
endfunction()

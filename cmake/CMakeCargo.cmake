function(cargo_build)
    cmake_parse_arguments(CARGO "BIN" "NAME" "" ${ARGN})
    string(REPLACE "-" "_" LIB_NAME ${CARGO_NAME})

    set(CARGO_TARGET_DIR ${CMAKE_BINARY_DIR}/target)

    if(WIN32)
        if(CMAKE_SIZEOF_VOID_P EQUAL 8)
            set(LIB_TARGET "x86_64-pc-windows-msvc")
        else()
            set(LIB_TARGET "i686-pc-windows-msvc")
        endif()
	elseif(ANDROID)
        if(ANDROID_SYSROOT_ABI STREQUAL "x86")
            set(LIB_TARGET "i686-linux-android")
        elseif(ANDROID_SYSROOT_ABI STREQUAL "x86_64")
            set(LIB_TARGET "x86_64-linux-android")
        elseif(ANDROID_SYSROOT_ABI STREQUAL "arm")
            set(LIB_TARGET "arm-linux-androideabi")
        elseif(ANDROID_SYSROOT_ABI STREQUAL "arm64")
            set(LIB_TARGET "aarch64-linux-android")
        endif()
    elseif(IOS)
		set(LIB_TARGET "universal")
    elseif(CMAKE_SYSTEM_NAME STREQUAL Darwin)
        set(LIB_TARGET "x86_64-apple-darwin")
	else()
        if(CMAKE_SIZEOF_VOID_P EQUAL 8)
            set(LIB_TARGET "x86_64-unknown-linux-gnu")
        else()
            set(LIB_TARGET "i686-unknown-linux-gnu")
        endif()
    endif()

    if(NOT CMAKE_BUILD_TYPE)
        set(LIB_BUILD_TYPE "debug")
    elseif(${CMAKE_BUILD_TYPE} STREQUAL "Release")
        set(LIB_BUILD_TYPE "release")
    else()
        set(LIB_BUILD_TYPE "debug")
    endif()

    set(LIB_FILE "${CARGO_TARGET_DIR}/${LIB_TARGET}/${LIB_BUILD_TYPE}/${CMAKE_STATIC_LIBRARY_PREFIX}${LIB_NAME}${CMAKE_STATIC_LIBRARY_SUFFIX}")
    set(BIN_FILE "${CARGO_TARGET_DIR}/${LIB_TARGET}/${LIB_BUILD_TYPE}/${LIB_NAME}${CMAKE_EXECUTABLE_SUFFIX}")

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

    set(
        CARGO_ENV_COMMAND ${CMAKE_COMMAND} -E env 
            "CMAKE_BINARY_DIR=${CMAKE_BINARY_DIR}"
            "CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/target"
    )

    if(${CARGO_BIN})
        set(OUTPUT_FILE "${CMAKE_BINARY_DIR}/_bin/${CARGO_NAME}${CMAKE_EXECUTABLE_SUFFIX}")

        add_custom_target(
            ${CARGO_NAME}_cargo ALL
            COMMAND ${CARGO_ENV_COMMAND} ${CARGO_EXECUTABLE} ${CARGO_ARGS}
            WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
            COMMENT "running cargo"
        )
        add_custom_target(
            ${CARGO_NAME}_copy ALL
            COMMAND cp -f ${BIN_FILE} ${OUTPUT_FILE}
            COMMENT "copying binary"
        )

        add_custom_target(${CARGO_NAME})

        add_dependencies(${CARGO_NAME} ${CARGO_NAME}_copy)
        add_dependencies(${CARGO_NAME}_copy ${CARGO_NAME}_cargo)
    else()
        set(OUTPUT_FILE ${LIB_FILE})

        add_custom_target(
            ${CARGO_NAME}_target
            COMMAND ${CARGO_ENV_COMMAND} ${CARGO_EXECUTABLE} ${CARGO_ARGS}
            WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
            COMMENT "running cargo")

        add_library(${CARGO_NAME} STATIC IMPORTED GLOBAL)
        set_target_properties(${CARGO_NAME} PROPERTIES IMPORTED_LOCATION ${LIB_FILE})
        
        add_dependencies(${CARGO_NAME} ${CARGO_NAME}_target)
    endif()

endfunction()
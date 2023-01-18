#!/bin/bash -eu

mkdir _build && cd _build
cmake -DHAVE_TEST=OFF \
	-DTARGET_SLIMRDS=OFF \
	-DHAVE_ASSERT_PANIC=OFF \
	-DTARGET_SLIMCACHE=OFF \
	-DTARGET_RDS=OFF \
	-DTARGET_TWEMCACHE=OFF \
	-DTARGET_PINGSERVER=OFF \
	 ..
make -j4

cd $SRC/pelikan/test/fuzz

$CC $CFLAGS -DOS_LINUX -DUSE_EVENT_FD \
        -D_FiILE_OFFSET_BITS=64 -D_GNU_SOURCE \
        -I../../deps/ccommon/include \
        -I../../deps/ccommon/include/buffer \
        -I../../src/protocol/data/resp \
        -I../../_build \
        -O2  -std=c11 -ggdb3  \
        -fno-strict-aliasing -O2  \
        -std=c11 -ggdb3 \
        -fstrict-aliasing  -O3 -fPIC \
        -o compiled.o -c fuzzer.c

$CC $CFLAGS $LIB_FUZZING_ENGINE compiled.o -o $OUT/fuzzer \
        ../../_build/protocol/data/resp/libprotocol_resp.a \
        ../../_build/core/libcore.a \
        ../../_build/ccommon/lib/libccommon-2.1.0.a

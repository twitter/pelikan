#!/bin/bash

rustup toolchain add nightly
cargo fuzz --version || cargo +nightly install cargo-fuzz

FUZZ_TIME=300
FUZZ_JOBS=2

cd ${CRATE_DIR}

if [ -e fuzz/dictionary/${FUZZ_TARGET}.dict ]; then
	cargo +nightly fuzz run ${FUZZ_TARGET} --jobs ${FUZZ_JOBS} -- -dict=fuzz/dictionary/${FUZZ_TARGET}.dict -max_total_time=${FUZZ_TIME}
else
	cargo +nightly fuzz run ${FUZZ_TARGET} --jobs ${FUZZ_JOBS} -- -max_total_time=${FUZZ_TIME}
fi

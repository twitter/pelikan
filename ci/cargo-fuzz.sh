#!/bin/bash

rustup toolchain add nightly
cargo fuzz --version || cargo +nightly install cargo-fuzz

PROJECT_DIR=`pwd`

cd src/rust/protocol
cargo +nightly fuzz run memcache --jobs 2 -- -dict=fuzz/dictionary/memcache.dict -max_total_time=300
cd $PROJECT_DIR

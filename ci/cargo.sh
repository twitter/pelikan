#!/bin/bash -e

cargo build
cargo test
cargo build --release
cargo test --release

# compile benchmarks to check for errors
cargo bench --no-run

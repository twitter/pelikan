#!/bin/bash

cargo build
cargo test
cargo build --release
cargo test --release

cargo test --bin pelikan_pingserver_rs --test integration

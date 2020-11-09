#!/bin/bash

cargo build
cargo test
cargo build --release
cargo test --release

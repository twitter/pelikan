#!/bin/bash

# This configuration is used by build and vendorizing scripts

export PROJECT_NAME="cdb_rs"
export TOOLCHAIN_VERSION="dev"
export VENDORED_SOURCES_SUFFIX="thirdparty"
export VENDORED_SOURCES_PACKER_ROLE="io-perf"
export CARGO_OPTIONS="--verbose --release"
export BUILD_TARGET="x86_64-unknown-linux-gnu"

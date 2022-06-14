#!/bin/bash

set -uo pipefail
IFS=$'\n\t'

# Cargo currently does not allow nested workspaces. Since this repo is vendored
# into github.com/twitter/pelikan which is itself a workspace, we cannot commit
# a workspace manifest for this repository. Cargo tracking issue: 
# https://github.com/rust-lang/cargo/issues/5042

# As a workaround, we can construct a workspace manifest before running the
# build. This allows the crates within this repo to share build artifacts.

###
# Create workspace manifest
###

cat > Cargo.toml <<HEADER
[workspace]
members = [
HEADER

for CRATE_MANIFEST in `find rust -type f -name "Cargo.toml"`; do
  CRATE_ROOT=`dirname ${CRATE_MANIFEST}`
  echo "\"${CRATE_ROOT}\"," >> Cargo.toml
done

cat >> Cargo.toml <<FOOTER
]

[profile.bench]
debug = true
lto = true
codegen-units = 1

[profile.release]
debug = true
lto = true
codegen-units = 1
FOOTER

###
# Run cargo build/test for the workspace
###

cargo build
cargo test
cargo build --release
cargo test --release

###
# Remove workspace manifest
###
rm Cargo.toml

#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

# for osx: 0. update brew; 1. install cmake if missing; 2. (gcc) unlink pre-installed gcc; 3. (gcc) install desired version of gcc

if [[ "$TRAVIS_OS_NAME" == "osx" ]]; then brew update; fi
if [[ "$TRAVIS_OS_NAME" == "osx" && -z "$(which cmake)" ]]; then brew install cmake; fi             # xcode 8.1 is missing cmake
if [[ "$TRAVIS_OS_NAME" == "osx" && "$COMPILER" == "gcc" ]]; then brew unlink gcc || true; fi       # ignore unlink errors
if [[ "$TRAVIS_OS_NAME" == "osx" && "$COMPILER" == "gcc" ]]; then brew unlink $FORMULA || true; fi  # ignore unlink errors
if [[ "$TRAVIS_OS_NAME" == "osx" && "$COMPILER" == "gcc" ]]; then brew install $FORMULA; fi

export CC="$C_COMPILER"

wget https://github.com/libcheck/check/releases/download/0.11.0/check-0.11.0.tar.gz
tar xvfz check-0.11.0.tar.gz
( cd check-0.11.0 && ./configure && make && sudo make install ) || die "installing check failed"

curl https://sh.rustup.rs -sSf | sh -s -- -y

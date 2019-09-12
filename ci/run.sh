#!/bin/bash

set -uo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

export PATH=$HOME/.cargo/bin:$PATH

cmake_cmd=(
  cmake
  -DBUILD_AND_INSTALL_CHECK=yes
)

if [[ -n "${RUST_ENABLED:-}" ]]; then
  cmake_cmd+=( -DHAVE_RUST=yes -DRUST_VERBOSE_BUILD=yes )
fi

export RUST_BACKTRACE=full

mkdir -p _build && ( cd _build && "${cmake_cmd[@]}" .. && make && make test )
RESULT=$?

egrep -r ":F:|:E:" . |grep -v 'Binary file' || true


if [[ $RESULT -ne 0 ]]; then
  echo "Build failure" >&2
  exit $RESULT
else
  echo "success!" >&2
  exit 0
fi

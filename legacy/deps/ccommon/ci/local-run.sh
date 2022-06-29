#!/bin/bash

## This file is potentially useful for doing a clean environment build on MacOS ##
## This is my personal way of running the build, YMMV - jsimms                  ##

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TOPLEVEL=$(git -C "$(cd "$(dirname "$0")" >/dev/null || exit 1; pwd)" rev-parse --show-toplevel) || die "TOPLEVEL fail"

cd "$TOPLEVEL"

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

BUILD_PATH=(
  "$HOME/.cargo/bin"
  "/usr/local/bin"
  "/usr/local/sbin"
  "/usr/bin"
  "/usr/sbin"
  "/bin"
  "/sbin"
  "/opt/X11/bin"
  "/usr/X11R6/bin"
)

PATH=$(echo "${BUILD_PATH[@]}"|tr ' ' ':')

cat >&2 <<EOS
CC=${CC:-}
CFLAGS=${CFLAGS:-}
LDFLAGS=${LDFLAGS:-}
PATH=${PATH:-}
EOS

CMAKEFLAGS=(
  -DHAVE_RUST=yes
  -DRUST_VERBOSE_BUILD=yes
  -DCMAKE_VERBOSE_MAKEFILE:BOOL=ON
)

BUILD_DIR="${BUILD_DIR:-$TEMP}"

mkdir -p "$BUILD_DIR" && (
  cd "$BUILD_DIR" &&
  cmake "${CMAKEFLAGS[@]}" "$TOPLEVEL" &&
  make all &&
  make check &&
  cd rust &&
  cargo test
)

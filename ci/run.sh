#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

export PATH=$HOME/.cargo/bin:$PATH

# build CDB in CI or else stuff breaks

CMAKE_ARGS=(
  -DBUILD_AND_INSTALL_CHECK=yes
)

if [[ -n "${RUST_ENABLED:-}" ]]; then
  CMAKE_ARGS+=( -DTARGET_CDB=yes -DHAVE_RUST=yes -DRUST_VERBOSE_BUILD=1 )
fi

# TODO: run cmake3 on centos hosts

mkdir -p _build && ( cd _build && cmake ${CMAKE_ARGS[@]} .. && make && make check ) || die 'make failed'

egrep -r ":F:|:E:" . |grep -v 'Binary file' || true

( cd test/integration && python test_twemcache.py ) || die 'twemcache tests failed'

set +e

if [[ -n "${RUST_ENABLED:-}" ]]; then
  ( cd src/storage/cdb && env RUST_BACKTRACE=full cargo test )
fi

RESULT=$?

if [[ $RESULT -ne 0 ]]; then
  echo "Build failure" >&2
  exit $RESULT
else
  echo "success!" >&2
  exit 0
fi

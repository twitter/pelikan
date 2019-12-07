#!/bin/bash

set -uo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

export PATH=$HOME/.cargo/bin:$PATH

# TODO: run cmake3 on centos hosts
cmake_cmd=(
  cmake
  -DBUILD_AND_INSTALL_CHECK=yes
  -DCARGO_CI=yes
)

# Ensure that rust test failures have the full backtrace
export RUST_BACKTRACE=full
# Make ctest display the output from failing tests
export CTEST_OUTPUT_ON_FAILURE=1

# build CDB in CI or else stuff breaks
if [[ -n "${RUST_ENABLED:-}" ]]; then
  cmake_cmd+=( -DTARGET_CDB=yes -DTARGET_HTTP=yes -DHAVE_RUST=yes -DRUST_VERBOSE_BUILD=yes )
fi

mkdir -p _build && ( cd _build && "${cmake_cmd[@]}" .. && make && make test ) || die 'make failed'

egrep -r ":F:|:E:" . |grep -v 'Binary file' || true

( cd test/integration && python test_twemcache.py ) || die 'twemcache tests failed'


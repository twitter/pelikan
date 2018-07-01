#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

export PATH=$HOME/.cargo/bin:$PATH

mkdir _build && ( cd _build && cmake .. && make -j && make check ) || die 'make failed'

egrep -r ":F:|:E:" . || true

( cd test/integration && python test_twemcache.py ) || die 'twemcache tests failed'
( cd src/storage/cdb && cargo test ) || die 'rust tests failed'

#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

export PATH=$HOME/.cargo/bin:$PATH

mkdir -p _build && ( cd _build && cmake .. && make -j && make check ) || die 'make failed'

egrep -r ":F:|:E:" . |grep -v 'Binary file' || true

( cd test/integration && python test_twemcache.py ) || die 'twemcache tests failed'

set +e

( cd src/storage/cdb && env RUST_BACKTRACE=full cargo test )

RESULT=$?

if [[ "$(uname -s)" == "Darwin" ]]; then
  if [[ $RESULT -ne 0 ]]; then
    echo "Rust test failed on OSX, but this does not fail the build" >&2
  fi

  exit 0
fi

if [[ $RESULT -ne 0 ]]; then
  echo "Build failure" >&2
  exit $RESULT
else
  echo "success!" >&2
  exit 0
fi

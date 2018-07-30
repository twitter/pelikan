#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

realpath() { python -c "import os,sys; print os.path.realpath(sys.argv[1])" "$1"; }

TOPLEVEL="$(cd "$(dirname "$(realpath "$0" >/dev/null || exit 1)")" && git rev-parse --show-toplevel)" || die 'failed to find TOPLEVEL'


# for osx: 0. update brew; 1. install cmake if missing; 2. (gcc) unlink pre-installed gcc; 3. (gcc) install desired version of gcc

if [[ "$TRAVIS_OS_NAME" == "osx" ]]; then
  brew update &>/dev/null
  brew install cmake || true            # xcode 8.1 is missing cmake

  if [[ "$C_COMPILER" =~ ^gcc && -n "${FORMULA:-}" ]]; then
    brew unlink gcc || true
    brew unlink "$FORMULA" || true
    brew install "$FORMULA"
  fi
fi

export CC="$C_COMPILER"

if [[ -n "${RUST_ENABLED:-}" ]]; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
fi

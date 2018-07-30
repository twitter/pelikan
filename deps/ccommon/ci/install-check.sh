#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 check-install-path"
  exit 1
fi

CHECK_PREFIX="$1"
shift

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

realpath() { python -c "import os,sys; print os.path.realpath(sys.argv[1])" "$1"; }

TOPLEVEL="$(cd "$(dirname "$(realpath "$0" >/dev/null || exit 1)")" && git rev-parse --show-toplevel)" || die 'failed to find TOPLEVEL'


CHECK_VERSION=0.12.0
CHECK_TARBALL="check-${CHECK_VERSION}.tar.gz"
CHECK_DIR="check-${CHECK_VERSION}"

(
  cd "$TEMP" &&
    wget "https://github.com/libcheck/check/releases/download/${CHECK_VERSION}/${CHECK_TARBALL}" &&
    tar xvfz "${CHECK_TARBALL}" &&
    cd "${CHECK_DIR}" &&
    ./configure --prefix="$CHECK_PREFIX" &&
    make &&
    make install
) || die "check build failed"

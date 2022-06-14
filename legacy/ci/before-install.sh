#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

realpath() { python -c "from __future__ import print_function; import os,sys; print(os.path.realpath(sys.argv[1]))" "$1"; }

TOPLEVEL="$(cd "$(dirname "$(realpath "$0" >/dev/null || exit 1)")" && git rev-parse --show-toplevel)" || die 'failed to find TOPLEVEL'


if [[ -n "${RUST_ENABLED:-}" ]]; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
fi

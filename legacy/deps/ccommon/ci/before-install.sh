#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

TOPLEVEL="$(git -C "$(cd "$(dirname "$0")" >/dev/null || exit 1; pwd)" rev-parse --show-toplevel)" || die 'failed to find TOPLEVEL'

if [[ -n "${RUST_ENABLED:-}" ]]; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
fi

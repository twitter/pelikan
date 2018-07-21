#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT


if [[ -n "$(git status --porcelain)" ]]; then
  die "dirty working copy state, please commit changes before running this script"
fi

git archive --format=tar --prefix=pelikan/ HEAD .|tar -C "$TEMP" -xf-
cd "$TEMP/pelikan"

bash ./ci/run.sh

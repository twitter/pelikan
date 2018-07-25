#!/bin/bash

# utility script run by cmake for writing a .cargo/config that points
# to the common 'target' directory under the CMAKE_BINARY_DIR. This
# allows automated tools (such as the intellij rust plugin or vcode
# rust integration) to share output and avoid recompiling between
# the command line and the IDE.
#
# it is assumed that this script is run with the CWD being the
# place where the .cargo dir should be created.

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

if [[ -z "${CMAKE_BINARY_DIR}" ]]; then
  die "CMAKE_BINARY_DIR must be set!"
fi

mkdir -p .cargo

cleanup() {
  [[ -n "${TEMPFILE:-}" ]] && rm -rf "$TEMPFILE"
}
trap cleanup EXIT

TEMPFILE="$(mktemp '.cargo/config.XXXXXXXX')" || die "could not create tempfile"

cat > "$TEMPFILE" <<EOS
[build]
target-dir = "${CMAKE_BINARY_DIR}/target"
EOS

mv "$TEMPFILE" .cargo/config

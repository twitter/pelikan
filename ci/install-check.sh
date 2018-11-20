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

TOPLEVEL="$(git -C "$(cd "$(dirname "$0")" >/dev/null || exit 1; pwd)" rev-parse --show-toplevel)" || die 'failed to find TOPLEVEL'

CHECK_VERSION=0.12.0
CHECK_TARBALL="check-${CHECK_VERSION}.tar.gz"
CHECK_DIR="check-${CHECK_VERSION}"

echo "building and installing check" >&2

(
  cd "$TEMP" &&
    wget "https://github.com/libcheck/check/releases/download/${CHECK_VERSION}/${CHECK_TARBALL}" &&
    tar xfz "${CHECK_TARBALL}" &&
    cd "${CHECK_DIR}" &&
    ./configure --prefix="$CHECK_PREFIX" &&
    make &&
    make install
) >$TEMP/cmake-build.log 2>&1

RESULT=$?
if [[ $RESULT -ne 0 ]]; then
  cat >&2 <<EOS
check build failed! log below:

EOS

  cat $TEMP/cmake-build.log
else
  echo "Success!" >&2
fi

exit $RESULT

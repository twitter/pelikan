#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 fluxcap-install-path"
  exit 1
fi

FLUXCAP_PREFIX="$1"
shift

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

realpath() { python3 -c "import os,sys; print(os.path.realpath(sys.argv[1]))" "$1"; }

TOPLEVEL="$(cd "$(dirname "$(realpath "$0" >/dev/null || exit 1)")" && git rev-parse --show-toplevel)" || die 'failed to find TOPLEVEL'


FLUXCAP_VERSION=0.1
FLUXCAP_TARBALL="${FLUXCAP_VERSION}.tar.gz"
FLUXCAP_DIR="fluxcapacitor-${FLUXCAP_VERSION}"
FLUXCAP_LOG="build-fluxcapacitor.log"

echo "building fluxcapacitor" >&2

(
  cd "$TEMP" &&
    wget "https://github.com/thinkingfish/fluxcapacitor/archive/${FLUXCAP_TARBALL}" &&
    tar xfz "${FLUXCAP_TARBALL}" &&
    pwd &&
    ls . &&
    cd "${FLUXCAP_DIR}" &&
    make build
) >$TEMP/${FLUXCAP_LOG} 2>&1

RESULT=$?
if [[ $RESULT -ne 0 ]]; then
  cat >&2 <<EOS
fluxcapacitor build failed! log below:

EOS

  cat $TEMP/${FLUXCAP_LOG}
else
  rsync -a "$TEMP/${FLUXCAP_DIR}/fluxcapacitor" ${FLUXCAP_PREFIX}/
  echo "Success!" >&2
fi

exit $RESULT

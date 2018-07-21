#!/bin/bash -e

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

# Script to produce a vendor tgz of external deps

# load variables
# shellcheck source=/dev/null
source scripts/config.sh

# cleanup old paths
rm -rf .cargo vendor tmp

# vendorize
cargo vendor --explicit-version vendor >/dev/null

# prepare temporary location
mv vendor "$TEMP/"
mkdir "$TEMP/.cargo"

# make cargo config
{
    echo '[source.crates-io]'
    echo 'replace-with = "vendored-sources"'
    echo ''
    echo '[source.vendored-sources]'
    echo 'directory = "./vendor"'
    echo ''
} >> "$TEMP/.cargo/config"

mkdir -p dist

TARBALL="dist/${PROJECT_NAME}"-"${VENDORED_SOURCES_SUFFIX}.tgz"

# create archive
tar -C "$TEMP" -czf "$TARBALL" .cargo vendor

# next-steps
echo 'upload to packer with:'
echo "package upload -r ${VENDORED_SOURCES_PACKER_ROLE} ${PROJECT_NAME}-${VENDORED_SOURCES_SUFFIX} ${TARBALL}"

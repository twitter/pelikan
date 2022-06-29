#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

die() { echo "fatal: $*" >&2; exit 1; }

TEMP="$(mktemp -d -t TEMP.XXXXXXX)" || die "failed to make tmpdir"
cleanup() { [[ -n "${TEMP:-}" ]] && rm -rf "${TEMP}"; }
trap cleanup EXIT

# This script installs a travis-ci docker image and attempts to run the build
# script in that container. in order for it to work you have to have the 'travis'
# command-line utility installed and the 'travis-build' plugin installed as well.
#
# instructions here: https://github.com/travis-ci/travis-build#use-as-addon-for-cli

CONTAINER_NAME='pelikan-travis-debug'

# this will undoubtedly need to be updated regularly
# see https://stackoverflow.com/a/49019950/965434 for more context
#
INSTANCE='travisci/ci-garnet:packer-1512502276-986baf0'


echo "cleanup previous run's container, no problem if failure" >&2

docker stop "$CONTAINER_NAME" || true
docker rm "$CONTAINER_NAME" || true

set -x

docker run --name "$CONTAINER_NAME" -dit "$INSTANCE" /sbin/init

CI_SCRIPT="$TEMP/ci.sh"

"$HOME/.travis/travis-build/bin/travis" compile > "$CI_SCRIPT"
chmod 755 "$CI_SCRIPT"

docker cp "$CI_SCRIPT" "$CONTAINER_NAME:/home/travis/ci.sh"

cat <<EO_RUN_SCRIPT >"$TEMP/run.sh"
#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

su - travis -- -c 'cd /home/travis && ./ci.sh'

EO_RUN_SCRIPT

docker cp "$TEMP/run.sh" "$CONTAINER_NAME:/run.sh"

docker exec "$CONTAINER_NAME" "/bin/bash" "/run.sh"

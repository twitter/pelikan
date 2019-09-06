#!/usr/bin/env bash

set -e
rm -rf lcov _build _bin && mkdir _build && pushd _build && cmake -DCMAKE_BUILD_TYPE=Debug -DCOVERAGE=on .. && make -j && make test && popd
mkdir lcov
lcov --directory . --capture --output-file lcov/app.info
genhtml lcov/app.info -o lcov/html

COVERAGE_FILE="lcov/html/index.html"
if which xdg-open; then
    OPEN=xdg-open
elif [[ $OSTYPE == "darwin"* ]]; then
    OPEN=open
else
    >&2 echo "Don't know how to automate open, coverage file is at: $COVERAGE_FILE"
fi
if [[ ! -z "$OPEN" ]]; then
    $OPEN $COVERAGE_FILE
fi
set +e

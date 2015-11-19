#!/usr/bin/env bash

set -e
rm -rf lcov _build _bin && mkdir _build && pushd _build && cmake -DCOVERAGE=on .. && make -j && make test && popd
mkdir lcov
lcov --directory . --capture --output-file lcov/app.info
genhtml lcov/app.info -o lcov/html
if [[ "$OSTYPE" == "darwin"* ]]; then
    open lcov/html/index.html
fi
set +e

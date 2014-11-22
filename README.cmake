# checkout ccommon source, it should be a sibling directory of broadbill
# otherwise the path needs to be explicitly set with `CCOMMON_SOURCE_DIR`

mkdir _build
cd _build
cmake ..
make

# binaries can be found at $(topdir)/bin/broadbill_*

# checkout ccommon source, it should be a sibling directory of broadbill
# otherwise the path needs to be explicitly set with `CCOMMON_SOURCE_DIR`

# to turn on/off various compile-time switches, use -D option with cmake
# Example:
#     cmake -DHAVE_LOGGING=OFF

mkdir _build
cd _build
cmake ..
make

# binaries can be found at $(topdir)/bin/broadbill_*

# ccommon is a submodule of broadbill. To learn about using submodules, follow
# this link: http://www.git-scm.com/book/en/v2/Git-Tools-Submodules

# to checkout ccommon source when checking out broadbill, use the following
git clone --recursive https://git.twitter.biz/broadbill
# to get an updated version of the submodule:
git submodule update --remote

# to turn on/off various compile-time switches, use -D option with cmake
# Example:
#     cmake -DHAVE_LOGGING=OFF

mkdir _build
cd _build
cmake ..
make

# binaries can be found at $(topdir)/bin/broadbill_*

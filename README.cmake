# all source code dependencies are under deps/, please read the README there for more information.
# current dependencies include Check for testing, and some of the usuals on UNIX-like systems:
# glibc, pthread and systems libraries.

# to turn on/off various compile-time switches, use -D option with cmake
# Example:
#     cmake -DHAVE_LOGGING=OFF

# To provide an alternative location of Check (C unit test framework used by this project), which is
# probably necessary if it is not installed under /usr/local, provide CHECK_ROOT_DIR to cmake
# Example:
#     cmake -DCHECK_ROOT_DIR=/opt/twitter ..

# cmake recommends out-of-source build, so we do it in a new directory:
mkdir _build
cd _build
cmake ..
make -j
make test

# executables can be found at $(topdir)/_bin/*

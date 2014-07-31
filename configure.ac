m4_define([BB_MAJOR], 0)
m4_define([BB_MINOR], 1)
m4_define([BB_PATCH], 0)
m4_define([BB_BUGS], [yao@twitter.com])

# Initialize autoconf
AC_PREREQ([2.60])
AC_INIT([broadbill], [BB_MAJOR.BB_MINOR.BB_PATCH], [BB_BUGS])
AC_GNU_SOURCE
AC_CONFIG_SRCDIR([src/bb_main.c])
AC_CONFIG_AUX_DIR([config])
AC_CONFIG_HEADERS([config.h:config.h.in])
AC_CONFIG_MACRO_DIR([m4])

# Checks for OS and set OS variables
AC_CANONICAL_HOST
case $host_os in
    linux*)   OS_LINUX=yes ;;
    darwin*)  OS_DARWIN=yes ;;
    *)        AC_MSG_ERROR([Your platform is not currently supported]) ;;
esac

# Initialize automake
AM_INIT_AUTOMAKE([1.10 check-news no-define foreign silent-rules subdir-objects
                  -Wall -Werror])
AM_SILENT_RULES([yes])

# Define macro variables for the package version numbers
AC_DEFINE(BB_VERSION_MAJOR, BB_MAJOR, [Define the major version number])
AC_DEFINE(BB_VERSION_MINOR, BB_MINOR, [Define the minor version number])
AC_DEFINE(BB_VERSION_PATCH, BB_PATCH, [Define the patch version number])
AC_DEFINE(BB_VERSION_STRING, "BB_MAJOR.BB_MINOR.BB_PATCH", [Define the version string])

# Checks for language
AC_LANG([C])

# Checks for programs
AC_PROG_CC
AC_PROG_MAKE_SET
AC_PROG_LIBTOOL
AM_PROG_CC_C_O

# Checks for typedefs, structures, and compiler characteristics
AC_C_INLINE
AC_C_CONST
AC_TYPE_INT8_T
AC_TYPE_INT16_T
AC_TYPE_INT32_T
AC_TYPE_INT64_T
AC_TYPE_INTMAX_T
AC_TYPE_INTPTR_T
AC_TYPE_UINT8_T
AC_TYPE_UINT16_T
AC_TYPE_UINT32_T
AC_TYPE_UINT64_T
AC_TYPE_UINTMAX_T
AC_TYPE_UINTPTR_T
AC_TYPE_OFF_T
AC_TYPE_PID_T
AC_TYPE_SIZE_T
AC_TYPE_SSIZE_T
AC_TYPE_UID_T

# Checks for header files

# Checks for libraries

# Checks for library functions
AC_FUNC_FORK

AC_C_BIGENDIAN(
  [],
  [AC_DEFINE(HAVE_LITTLE_ENDIAN, 1, [Define to 1 if machine is little endian])],
  [AC_MSG_ERROR([endianess of this machine is unknown])],
  [AC_MSG_ERROR([universial endianess not supported])]
)

## Check whether to enable debug logs and asserts
AC_MSG_CHECKING([whether to enable debug logs and asserts])
AC_ARG_ENABLE([debug],
  [AS_HELP_STRING(
    [--enable-debug=@<:@panic|assert|log|no@:>@],
    [enable debug logs and asserts @<:@default=no@:>@])
  ],
  [],
  [enable_debug=no])
AS_CASE([x$enable_debug],
  [xpanic], [AC_DEFINE([HAVE_ASSERT_PANIC], [1],
                      [Define to 1 if panic on an assert is enabled])
            AC_DEFINE([HAVE_DEBUG_LOG], [1], [Define to 1 if debug log is enabled])
           ],
  [xassert], [AC_DEFINE([HAVE_ASSERT_LOG], [1],
                     [Define to 1 if log on an assert is enabled])
           AC_DEFINE([HAVE_DEBUG_LOG], [1], [Define to 1 if debug log is enabled])
          ],
  [xlog], [AC_DEFINE([HAVE_DEBUG_LOG], [1], [Define to 1 if debug log is enabled])],
  [xno], [],
  [AC_MSG_FAILURE([invalid value ${enable_debug} for --enable-debug])])
AC_MSG_RESULT([$enable_debug])

AC_MSG_CHECKING([whether to disable stats])
AC_ARG_ENABLE([stats],
  [AS_HELP_STRING(
    [--disable-stats],
    [disable stats])
  ],
  [disable_stats=yes],
  [disable_stats=no])
AS_IF([test "x$disable_stats" = xyes],
  [],
  [AC_DEFINE([HAVE_STATS], [1], [Define to 1 if stats is not disabled])])
AC_MSG_RESULT($disable_stats)

# Check whether the test libraries are present
PKG_CHECK_MODULES([CHECK], [check >= 0.9.10])
PKG_CHECK_MODULES([CCOMMON], [ccommon-1.0 >= 1.0])

# Define Makefiles
AC_CONFIG_FILES([Makefile
                 src/Makefile])


# Generate the "configure" script
AC_OUTPUT

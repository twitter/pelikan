AM_CPPFLAGS =
AM_CPPFLAGS += -I $(top_srcdir)/src

AM_CFLAGS =
AM_CFLAGS += -Wall -Wshadow
AM_CFLAGS += -Wpointer-arith
AM_CFLAGS += -Winline
AM_CFLAGS += -Wunused-function -Wunused-variable -Wunused-value
AM_CFLAGS += -Wno-unused-parameter -Wno-unused-value
AM_CFLAGS += -Wconversion -Wsign-compare
AM_CFLAGS += -Wstrict-prototypes -Wmissing-prototypes -Wredundant-decls -Wmissing-declarations


TESTS = 				\
	check_memcache

check_PROGRAMS =		\
	check_memcache

check_memcache_SOURCES = check_memcache.c $(top_builddir)/src/bb_memcache.h
check_memcache_CFLAGS = @CHECK_CFLAGS@

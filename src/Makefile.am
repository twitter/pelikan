MAINTAINERCLEANFILES = Makefile.in

AM_CPPFLAGS =
AM_CPPFLAGS += -I $(top_srcdir)/src
AM_CPPFLAGS += -I $(top_srcdir)/src/memcache

AM_CFLAGS =
AM_CFLAGS += @CCOMMON_CFLAGS@
AM_CFLAGS += -Wall -Wshadow
AM_CFLAGS += -Wpointer-arith
AM_CFLAGS += -Winline
AM_CFLAGS += -Wunused-function -Wunused-variable -Wunused-value
AM_CFLAGS += -Wno-unused-parameter -Wno-unused-value
AM_CFLAGS += -Wconversion -Wsign-compare
AM_CFLAGS += -Wstrict-prototypes -Wmissing-prototypes -Wredundant-decls -Wmissing-declarations

AM_LDFLAGS =
AM_LDFLAGS += @CCOMMON_LIBS@
#AM_LDFLAGS += -rdynamic


bin_PROGRAMS = slimcache

slimcache_SOURCES = 							\
		memcache/bb_request.c memcache/bb_request.h		\
		bb_main.c

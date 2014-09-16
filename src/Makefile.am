MAINTAINERCLEANFILES = Makefile.in

AM_CPPFLAGS =
AM_CPPFLAGS += -I $(top_srcdir)/src
AM_CPPFLAGS += -I $(top_srcdir)/src/cuckoo
AM_CPPFLAGS += -I $(top_srcdir)/src/memcache
AM_CPPFLAGS += -I $(top_srcdir)/src/time

AM_CFLAGS =
AM_CFLAGS += @CCOMMON_CFLAGS@
AM_CFLAGS += -Wall -Wshadow
AM_CFLAGS += -Winline
AM_CFLAGS += -Wunused-function -Wunused-variable -Wunused-value
AM_CFLAGS += -Wno-unused-parameter -Wno-unused-value
AM_CFLAGS += -Wconversion -Wsign-compare
AM_CFLAGS += -Wstrict-prototypes -Wmissing-prototypes -Wredundant-decls -Wmissing-declarations
AM_CFLAGS += -fno-strict-aliasing

AM_LDFLAGS =
AM_LDFLAGS += @CCOMMON_LIBS@
#AM_LDFLAGS += -rdynamic


bin_PROGRAMS = broadbill_slimcache

broadbill_slimcache_SOURCES = 						\
		cuckoo/bb_cuckoo.c cuckoo/bb_cuckoo.h cuckoo/bb_item.h	\
		memcache/bb_constant.h					\
		memcache/bb_request.c memcache/bb_request.h		\
		memcache/bb_codec.c memcache/bb_codec.h			\
		memcache/bb_response.h					\
		slimcache/bb_process.c slimcache/bb_process.h		\
		slimcache/bb_core.c slimcache/bb_core.h			\
		time/bb_time.c time/bb_time.h				\
		bb_main.c

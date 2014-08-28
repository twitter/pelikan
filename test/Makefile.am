AM_CPPFLAGS =
AM_CPPFLAGS += -I$(top_srcdir)/src

AM_CFLAGS =
AM_CFLAGS += -Wall -Wshadow
AM_CFLAGS += -Wpointer-arith
AM_CFLAGS += -Winline
AM_CFLAGS += -Wunused-function -Wunused-variable -Wunused-value
AM_CFLAGS += -Wno-unused-parameter -Wno-unused-value
AM_CFLAGS += -Wconversion -Wsign-compare
AM_CFLAGS += -Wstrict-prototypes -Wmissing-prototypes -Wredundant-decls -Wmissing-declarations

TESTS = 						\
	check_tests

check_PROGRAMS =					\
	check_tests

check_tests_SOURCES =	 				\
	check_memcache.c				\
	$(top_srcdir)/src/memcache/bb_request.h

check_tests_CFLAGS = @CHECK_CFLAGS@ @CCOMMON_CFLAGS@
check_tests_LDADD = @CHECK_LIBS@ @CCOMMON_LIBS@ \
		       $(top_srcdir)/src/memcache/bb_request.o

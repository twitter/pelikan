MAINTAINERCLEANFILES = Makefile.in aclocal.m4 configure config.h.in config.h.in~ stamp-h.in

ACLOCAL_AMFLAGS = -I m4

SUBDIRS = src . test

EXTRA_DIST = README README.md NOTICE LICENSE ChangeLog scripts notes

test:
	@echo This will run unit tests powered by Check.
	make check

This module reimplements part of Redis's ziplist with some API modifications.
Most notably, the APIs are adjusted such that none of them tries to allocate or
free any memory.

The format of the data structure is also changed significantly under some
different assumptions, read the header and source file comments to learn the
details.

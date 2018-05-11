This module reimplements part of Redis's ziplist with some API modifications.
Most notably, the APIs are adjusted such that none of them tries to allocate or
free any memory.

The format of the data structure itself sticks to what is described in the
Redis implementation and therefore a copy of the explanation is included in
the source file. The rest of the code are rewritten instead of copied verbatim.

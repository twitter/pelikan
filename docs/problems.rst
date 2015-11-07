***********************
Problems and Priorities
***********************

When we talk to people about a unified caching solution, one question that gets
asked a lot is "why not just use Redis?"

It is easy to dismiss memcached and Twemcache because the protocol is
essentially designed for simple key with flat values, but indeed, what about
Redis? In terms of features- what can the cache backend do?- Redis is extremely
rich and covers almost all of our typical use cases. However, Redis has its own
problems when it comes to how things are implemented.

In the end, we feel we need stronger support on several production and
scalability-related issues.

Logging
=======

Logging is often used to answer the important question "what happened?" This is
very valuable in debugging production systems, and many logging libraries
emphasize preserving log messages that are linked to errors and critical systems
conditions. However, there's another much less mentioned role logging can serve-
to answer the question "how does the system work?" One of the biggest barriers
for someone to come to a new codebase and become productive is to understand how
things are connected, especially in the lack of good, high-level documentation.
But proper logging can reveal the flow of the logic easily to anybody who cares
to read the messages. Unfortunately, this aspect of logging is rarely used,
often because many services don't have a consistent logging practice.

A big reason for relatively sparse logging is the overhead- actually logging
something takes formatting a string and making a syscall to write to some file.
The former is not free but mostly predictable, but the latter, even mostly
buffered, can lead to unpredictable latencies when disk IO is contended. In
fact, we have seen logging slowing down the Twemcache backend in production
when the throughput is high and some background activities created contention
for the disk, and we were logging nowhere near the "verbose" level. As a result
of these side effects, we almost can never get very informative logs in
production when we need them, which makes debugging difficult and slow, with a
lot of guessing and testing involved.

Observability
=============

Stats are paramount to large-scale deploy. Health of the entire cluster, or in
fact many clusters, is automatically monitored via reading and interpreting
system and service stats. The provisioning of consistent, useful stats in Redis
does not meet our production requirement, even in the latest versions.
Furthermore, stats in all our current backends are kept in a monolithic
structure that is fixed for the service, meaning anybody who wants to use part
of the code base has to replicate the monitoring aspect, or altering the code
to achieve that. We found that limiting and cumbersome to our unification
effort.

Resource Management
===================

In production especially a container-like environment where quotas of all sorts
are enforced, it is very important to maintain deterministic runtime behavior,
and avoid throttling, latency outliers and over-allocating memory and other
important resources. Redis delegates memory allocation to libraries such as
jemalloc, and when both large and small objects are allocated to heap, the time
it takes to acquire the next allocation can be unpredictable, as well as the
heap fragmentation ratio. On the other hand, none of the existing cache backends
put any constraint on how many connections can be maintained concurrently, which
in a sufficiently large cluster can lead to memory bloat caused by connection
storm (due to many reasons, expansion and connectivity being two of them),
sometimes getting the entire job/container killed. Both of these uncertainties
make capacity planning a lot harder, and increase resource waste by requiring
wide margins.

Protocol Extensibility
======================

Both Memcached and Redis are simple, plain-text based protocols with relatively
fixed semantics. To add a new command, both client and server have to understand
it, if there is a proxy along the way, the proxy has to as well. If we want to
apply back pressure, a common strategy to cope with hot keys and overloaded
backends, the existing protocols leave us no room to "tag-along" a flag easily
and idiomatically. As we try to develop cache to support more use cases and
intelligently handle larger scale clusters, the limitation of the simple
protocols become more and more apparent over time.

Solutions
=========

Of course, these are all solvable problems. The question then became: should we
solve them within the framework of existing solutions, or should we create
something new? In the end, what we have in mind looks sufficiently different
from the existing solutions structure-wise, which makes a new codebase more
reasonable. Solving these problems continue to serve as the primary motivation
and priorities of the project.

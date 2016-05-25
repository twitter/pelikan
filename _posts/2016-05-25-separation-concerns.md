---
layout: post
title:  "Separation of Concerns"
date:   2016-05-25 00:20:00 -0700
author: Yao Yue
tags: design, performance, operations
twitter_username: thinkingfish
---

*This is the third post in our [blog series](http://twitter.github.io/pelikan/blog/)
about the design, implementation and usage of caching in datacenters.*

The most important design decision we adopted in building the server is to
separate performance-sensitive processing from the rest, and separate different
types of performance-sensitive processing from each other. This is key to ensure
deterministic runtime behavior facing a wide range of environments and workloads.
As Dean et al have pointed out in [the Tail at Scale](http://www.cs.duke.edu/courses/cps296.4/fall13/838-CloudPapers/dean_longtail.pdf),
performance variability is amplified by scale, and the key to reduce
(meaningful) variability is differentiation.

## Issues observed

While operating Twemcache and Redis in production, we have seen:

* logging to disk impeding request processing on a particular thread;
* a flood of new connections, common after some network "blip", virtually
  prevent a server from processing any requests for extended period of time;
* probing a server for either health or stats becomes unreliable when it is
  heavily loaded.

Such unpredictable behavior has real consequences in production:

* we cannot enable detailed logging by default due to performance concerns;
* network glitches can bring down a server unnecessarily, even when the network
  failure itself is rather brief and recoverable;
* health of a server can be misjudged, resulting in the restart of an loaded
  server, leading to even more load when the server comes back or additional
  load elsewhere.

Most of such uncertainties can be avoided, if we carefully separate operations
that may interfere with each other undesirably. And of course, we are not the
first to apply this principle. Look no further than right under our feet for an
example – computer networks, being one of the earliest and most important type
of large-scale distributed systems, have given this problem plenty of thoughts.

## Data Plane, Control Plane

Networking provides the substructure to most distributed systems. To maximize
throughput, minimize latencies and jitter, networking technologies in recent
decades adheres to a divide between *control plane* and *data plane*[^1]. Data
plane is in charge of actually forwarding individual packets, and its
performance is directly measurable by the end users, such as when you send a
`ping` and wait for the response. Control plane deals with uncommon events,
such as a packet than cannot be routed, or recomputing routing table upon
topology changes. Unsurprisingly, a trip through the data plane is called the
"fast path" while landing on the control plane puts a packet through the "slow
path". Data plane optimizes for latency and throughput, often implementing a
relatively fixed pipeline using ASIC or simple, fast processors. Control plane
emphasizes more on flexibility, and is often equipped with general-purpose
processor(s) capable of running software that can be easily updated. There is
often a several orders of magnitude difference between these two planes in terms
of throughput.

The computer networking community demonstrated that by recognizing difference in
priorities for different parts of the system, they can make packet processing
fast while keeping state of the system well-managed and flexible. We take that
lesson and apply the same analogy to high-performance caching systems.

## Processing Pipelines in a Cache Server

The core idea of the networking model is minimizing work and interference on
performance-critical paths, and allow processing to flow unobstructed. The first
step in applying such a strategy is to recognize all the possible processing
pipelines in a cache server, and label the performance critical ones.

### Request, Response

The most common case in a cache server is the `request→response` pipeline. A
request is sent over an established channel such as a TCP connection, and the
server processes the request and sends back a response, usually over the same
channel. This accounts for the vast majority of the load under normal
operations, and is definitely performance-sensitive.

### Channels Establishment

Since a cache server in a datacenter almost always runs in a different process,
and usually on a different host from the clients, communication channels need to
be established before handling requests. This pipeline has less stringent
requirements on throughput and latency, since most channels are kept around
through many requests-responses cycles, so the cost is amortized. Still, it adds
to the perceived latency of the first request, and concurrency can rise
unexpectedly through synchronized client behavior after deploy and network
glitches. This pipeline is therefore still performance-sensitive, but should
give precedence to the `request-response` pipeline.

### Monitoring and Administration

To use a (cache) server with any seriousness requires some amount of
administrative capabilities, which include but are not limited by: querying
server metadata, monitoring its health and condition, logging and rotating logs,
updating certain configuration options without restarting. These are important
features but quite tolerant performance-wise. For example, having debug logs
synced to disk every second instead of every 100 millisecond is unlikely to be
noticeable by debuggers or operators; having metrics exported with a 200
millisecond delay instead of 20 makes little difference in monitoring quality.

Because the velocity of these functionalities are much lower and their latency
requirements lenient, they usually can be processed using time-shared resources.
The only exception would be background tasks that generate sufficient load but
are otherwise latency insensitive, such as snapshot/backup.

## Avoid Things That *Could* Be Slow

To keep slow things out of fast paths, we need to identify what those things
are. Some are more obvious– for example, one quickly learns not to use blocking
sockets in a single-threaded server. Other operations are better at hiding their
performance woes.

Many operations are *usually* fast but not always so, these operations are much
harder to avoid upfront because tests or even benchmarks may give the illusion
that everything is running as smoothly as needed. Even when running in
production, the server may behave mostly fine, but showing occasional slow-downs
that can be quite "mysterious" and hard to reproduce. Such performance issues
tend to become bigger and more constant headaches as one scales up their
operations, but remains hard to debug.

We certainly went through some of these headaches over the years, as Twitter
cache went from running a few dozen to tens of thousands of instances. Each
time significant time and resources were put into tracking down the root cause.
Each time the debugging process was both fascinating and demanding.
Unfortunately, not every problem had a simple fix within the existing
architecture, and that was an important motivation for us to redo the design
through Pelikan.

Because many such operations are deep in the heart of service implementation, I
believe it is worthwhile to call out these subtleties. Hopefully, future
developers will be aware of them upfront and avoid the same pitfalls.

### Write to file

File I/O these days are almost always buffered, unless one explicitly calls
`fsync`. As a result, a call to `write` (and its siblings) almost always returns
immediately, since all that the kernel does is moving some bytes in memory.
Furthermore, if the file descriptor corresponds to a non-blocking I/O object,
such as a socket, the call *always* returns immediately, deepening the
impression that `write` is fast.

However, the latency of `write` is not guaranteed. If the buffer is full and the
file is backed by disk, `write` can implicitly trigger sync to flush data from
buffer(s) to disk while users have no direct control over this mechanism. At
Twitter, we only realized this was a problem for Twemcache after observing
sporadic spikes in tail latencies on some of our busiest cache servers. Since
disk activities are not logged by default, we had to sift through a much wider
range of application logs to find correlation. Eventually a pattern emerged,
where we notice latency spikes were observed only when certain I/O-intensive
application were activated through cron. Still, presumably Twemcache stored
everything in memory and swap was disabled on these, so when would we ever go
to disk? The culprit turned out to be a small change in the rather innocent
looking log utility, a standard part of most production services. Shortly before
the symptoms appeared, we had increased our log level slightly to study
connection activities. After the incident, we had to turn the log level back
down to avoid performance hiccups in cache and further upstream.

### Locking

Most developers are aware of the effect of lock contention on performance, but
not necessarily the *extent* of it. Again, this is because contention is low
most of the time, where performance is largely predictable. However, tail
latencies tend to spin out of control when a server using locking is
[hit hard](http://blog.tsunanet.net/2010/11/how-long-does-it-take-to-make-context.html),
especially when the locking mechanism also leads to heavy context switching,
such as those using [`futex`](https://www.akkadia.org/drepper/futex.pdf).
When we looked at the impact of locking to performance in Twemcache[^2], it was
evident that it hinders scalability and tail latencies dramatically.

### Syscalls

One thing that is somewhat unique to cache server is how little work the server
needs to do to fulfill a request.  And most of the heavy-lifting is done by
syscalls – when we profiled Twemcache[^2], we noticed almost 80% of CPU time
went to syscalls and is spent in kernel space.

#### Requests
Without pipelining, a simple read request over a socket involves the following
steps:

1. an event syscall to notify data arrival on the socket (cost of this call is
  amortized over all the events returned at the same time);
2. a syscall to read from the socket;
3. processing request in user space;
4. a syscall to write response to the socket.

That amounts to 2 to 3 syscalls plus application logic processing the request,
which often is a simple hash lookup followed by by probing a small memory
region. If requests are pipelined, the cost of read / write can be further
amortized over all multiple requests.

#### Connections
In comparison, connection establishment is more syscall-intensive:

1. it also starts with an event syscall that returns activity on the listening
  socket (cost of this call is amortized over concurrent connection requests);
2. a syscall to accept the connection;
3. one or more syscalls to set socket as nonblocking unless a more efficient API
  such as `accept4` is available, other attributes such as `keepalive` still
  need to be set separately;
4. another syscall to add the socket needs to the right event loop, if the right
  event loop runs on a different thread, inter-thread communication is required.

That amounts to at least 2 syscalls per connection, but often quite a few more.
It cannot resort to pipelining to use syscalls more economically.

#### Implication
The effect of relatively expensive connections establishment is that when
hosting a large number of clients, the clients can easily DDoS the server by
synchronizing their connecting attempts (the TCP handshake also puts
a lot of pressure inside the kernel stack, which we will not go into here). The
situation is greatly exacerbated if the same thread is responsible for both
connection establishment and request handling.

## A Performance-Oriented Architecture

### One thread per processing pipeline

The first and most important decision in such an architecture is to assign
functionalities to either data plane or control plane– request-response and
connection establishment belong to data plane and need to be fast, while the
rest should go to control plane. Furthermore, we give each major processing
pipeline its own thread. For a simple in-memory cache implementation like
`pelikan_twemcache` or `pelikan_slimcache`, we use three threads:

* **Worker thread**: worker thread handles all latency-sensitive data requests,
  such as `get`, `set`, but is not responsible for those related to
  administrative tasks, such as `stats`. Worker thread is also off the hook from
  accepting connections, but still needs to register connections for event
  notifications.
* **Server thread**: server thread listens on the advertised data port and
  accepts (or rejects, when necessary) incoming connection requests. It should
  be mostly idling when connections are stable and reused, but can handle big
  spikes of new connection requests.
* **Admin thread**: admin thread does all the housekeeping: it listens on a
  separate control plane port ("admin port") to avoid mixing data plane traffic
  with control plane, accepts connections, answers requests regarding service
  status, and periodically aggregates metrics, flushes logs, etc.

### Performance-sensitive threads should not block

Knowing what operations could be slow, the next thing is to make sure we do not
invoke them inside processing pipelines that are performance-sensitive:

* No explicit or implicit use of syscalls that may block: worker and server
  thread should not use logging implementations that implicitly calls `write`.
  Instead, an in-memory buffer is used for writes and admin thread is responsible
  for reading from the buffer and writing its content to persistent storage.
  Memory allocation should also be used judiciously, as `malloc` could have
  unbounded worst-case latencies.
* Minimal communication between threads with lightweight synchronization:
  core data structures should have a clear owner thread for each operation
  type (read, write), and avoid synchronization as much as possible. When
  communication is necessary, such as connection handover between server/worker
  threads, prefer asynchronous data structures and mechanisms such as pipes and
  events. *futex*-based primitives should be used sparingly, in favor of lighter
  weight alternatives, such as atomic instructions. For example, metric
  operations can be entirely carried out with atomic instructions, so worker
  thread can update them and admin thread can read them without locking.

## Coming up...

We are going to talk about Pelikan's memory management strategy, another core
design decision, in the next post.

[^1]: [The Control Plane, Data Plane and Forwarding Plane in Networks](http://networkstatic.net/the-control-plane-data-plane-and-forwarding-plane-in-networks/)
[^2]: [Profiling Twemcache](https://github.com/twitter/twemcache/wiki/Impact-of-Lock-Contention)

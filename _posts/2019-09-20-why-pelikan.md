---
layout: post
comments: true
title:  "Why Pelikan"
date:   2019-09-20 02:00:00 -0700
author: Yao Yue
tags: design, security, performance, operations
twitter_username: thinkingfish
---

*This post was originally written as an internal document. Some links, project
names, and content are removed when adapting to a public audience.*

## TL;DR

Twemcache and Redis both solve some subset of caching problems at Twitter, and
neither does it perfectly. Pelikan addresses many of the issues that cause
unpredictable performance, difficulty with debugging, and other operational
headaches plaguing existing solutions, while cutting down maintenance burden by
consolidating everything into a single source repository. Pelikan achieves these
by treating cache server as a framework, abstracting common functionalities as
modules, and implementing differentiating features against common interfaces.
Pelikan’s modular design allows for much faster feature development in our
experience. Benchmarking has shown it to be more CPU efficient while yielding
much lower tail latencies. Migration to Pelikan is easy, because it is fully
API-compatible with Twemcache and Redis, and can be used as a drop-in
replacement for both. While there is always risks associated with any new
software, the benefits Pelikan provides over status quo makes it worth pursuing.

## Overview

Despite the availability of Memcached/Twemcache and Redis, both highly popular
and seemingly mature projects, existing solutions don’t really fully answer
the cache requirements and challenges Twitter faces. We are currently stuck
maintaining two independent software stacks while leaving gaps in functionality
and reliability. Meanwhile, Twitter services’ scalability and efficiency are
highly reliant on high-quality cache offerings, and there is additional
productivity and insight that the cache service can offer to Twitter’s product
and infrastructure. This doc aims to explain why a *Modular Cache Architecture*
can better serve the customers of cache, and improve productivity on
cache-related feature.

## Issues with Status Quo

### Common Problems with Existing Backends

- Limited or no protocol extensibility to introduce features commonly found in
  many other Twitter services, such as back-pressure, optional attributes,
  versioning, etc.
- Lack of pooling and preallocation for common resources, which means
  unpredictable request latencies and/or a surge of memory footprint (even OOM)
  under traffic pattern shift.
- High metadata overhead for small items. Twemcache incurs a fixed 49 bytes of
  overhead per item (57 w/ use of `cas`); Redis’ per-key memory overhead is even
  higher. For small items (which Twitter has a ton), this is quite wasteful and
  can lead to having over 50% of memory used for overhead alone.
- Operations like stats reporting and connections cleanup are handled by the
  same thread that processes latency-sensitive requests. This subjects the
  server performance to additional external inputs, and leads to unpredictable
  performance and potential loss of visibility under high load.
- Reliability vulnerability: commands, e.g. `flush_all` (Twemcache) or
  `FLUSHALL`/`FLUSHDB` (Redis) which can wipe out the entire dataset are not
  treated as privileged commands and are exposed to anybody who can access the
  host and port.
- Poor abstraction leading to difficulty in code change or reuse: low level
  network I/O objects are passed around and visible to logic that should only
  handle storage or data transformation.

### Problems with Redis

- Lack of explicit memory management can lead to unbound fragmentation even with
  a good `malloc` library. In practice fragmentation is all over the place,
  leading to reliability issues such as OOM.
- One thread per instance handling connections, requests, and maintenance.
  Overloaded thread means request latencies can be affected by connection storm,
  evictions etc. It limits visibility or other features useful for production
  health due to (fear of) contention.
- Loss of generality: latency and memory issues required users adopting over-
  allocation, config hacks or patches as one-off fixes, preventing fully
  automated, predictable deploy.
- The protocol is not extensible within each API. Optional fields are not an
  option.
- Poor logging and stats support, which are also hard to fix without
  exacerbating performance issues.
- Expiration behavior is unpredictable and subject to tuning. Some Redis
  clusters have poor memory utilization or OOM unexpectedly. The behavior can
  also change between versions. E.g.  memory utilization degraded in some
  clusters when we moved to Redis 3.2.

### Problems with Twemcache

- Lack of data structure support means it cannot cover all of Twitter's typical
  cache use cases.
- Memcached ASCII syntax makes is difficult to represent complex request
  semantics such as those needed for data structure manipulation.
- The multi-threaded implementation uses multiple locks and can block (e.g. when
  logging), leading to occasional latency spikes and poor latency under load. In
  general, its multithreading is not designed for modern many-core architecture.

### GDPR Impact

Certain aspects of these problems are exacerbated under GDPR.

The Service Authentication requirements, implemented as mutual TLS with key
distribution support, greatly increases the overhead of connection
establishment. This makes the Redis threading model prone to large latency
spikes during maintenance and other server/client churns. Twemcache will require
major retrofit to tear down the SSL context properly without affecting tail
latencies.

Audit Logging for services requires us to at least log and aggregate connection
activities, and maybe even per-request logging if the situation warrants it. A
wait-less logging module becomes mandatory for this scenario, as past cache
incidents showed that even directly logging connection activities to disk can
lead to unacceptable latencies. Such a setup is already in place for Pelikan for
all types of logging, can be constructed in Twemcache albeit somewhat messily,
and is straight out impossible in Redis.

## Pelikan Highlights

The goals of Pelikan are three folds:
1. **Performance and reliability**: best-in-class efficiency and predictability
  through latency-oriented design and lean implementation.
2. **Productivity**: a highly modularized code base that allows much faster
  feature development.
3. **Operational excellence**: rich configuration with simple syntax, fully
  automated deploy, full-stack visibility that can be turned on even under
  stringent production requirements.

### Performance and Reliability

The way Pelikan achieves excellent, predictable runtime behavior is through
improved architecture, including a [clean thread model](http://pelikan.io/2016/separation-concerns.html),
deterministic memory allocation, wait-less logging/stats, and other carefully
chosen design patterns. For more details, see [this talk](https://www.infoq.com/presentations/pelikan)
at QConSF 2016.

  ![pelikan_twemcache_latency]({{ "/assets/img/pelikan_twemcache_latency.jpg" | prepend: site.baseurl }})

Side-by-side benchmarking using an identical [rpc-perf](https://github.com/twitter/rpc-perf)
setup that emulates cache traffic to a major cache cluster shows that Pelikan
has much lower (~40% lower at `p99+`) and more predictable tail latencies than
Twemcache at the same throughput. This experiment was done with 50k QPS and 4
GiB heap size, 32 byte payloads, and about 5k connections per backend.

Early benchmarking also showed Pelikan improved throughput by 15% compared to
Twemcache.

### Improve Productivity

One common dilemma faced by infrastructure teams is the tension between
"one-size-fits-most" solution and highly specialized services. The former has
operational and maintenance advantage, while the latter often yields gains in
cost and performance. It is challenging for solutions in a broader problem
space such as storage to strike a good balance, and the path Twitter took
showed swings between these two extremes.

The same dilemma is present in the cache problem space as well. For example,
a large percentage of Twitter's cached key-values are tiny, and we can
theoretically cut per-key metadata overhead by 90%. In other cases, large,
rarely-updated data can be served more cheaply from SSD. In the past, Cache
team built prototypes such as `slimcache` and `fatcache` with excellent
benchmark numbers, but neither went into production because it is burdensome for
a small team to support even more codebases.

Pelikan emphasizes abstraction especially modules and frameworks, minimizing the
surface area and code for new features. For example, adding Cuckoo hashing as a
storage module involved <1000 LOC, and the development was done in about a week.
Linking it to existing modules gives us a new binary, `pelikan_slimcache`, that
looks and operates almost exactly the same as `pelikan_twemcache`. Similarly,
adding the RESP protocol (needed for Redis) was also <1000 LOC. Smaller code
difference translates to proportionally small config changes. As a result, a
single automation workflow and script set can handle multiple binaries. This
makes it easy to support specialized backends with modest incremental change.

Eventually, cache will need to address the interface limitations that leave the
cache services divided between two stacks, while offering a feature set less
reliable and rich than could be. API unification and improvement can only be
done through a major redesign of the protocol. Pelikan sets the stage for this
future development by making sure we can test and continue to support all the
other functionalities when introducing the new interface, and guarantee largely
unchanged runtime characteristics with architectural stability.

### Operational Excellence

#### Visibility

A operation-friendly service has to provide good introspective visibility,
primarily via metrics and logging. In this regard, Pelikan is a major step-up
from Twemcache and Redis.

Pelikan provides full metric/logging coverage of all modules, with built-in
documentation and highly regular naming. It offers 50%+ more metrics and 2x more
logging than Twemcache; and several times more metrics and 5x more logging than
Redis.

Pelikan does so without the performance penalty typical of visibility that is
added as a last-minute afterthought. Metrics are updated without locking, and
logging is non-blocking. Both can be turned on, dialed up at full throttle in
production without fearing of causing an incident like Twemcache/Redis could.

#### Automation

Automation is a pillar of contemporary operations engineering. A less mentioned
fact, though, is its feasibility depends on designing services with operational
requirements in mind.

Because cache team avoids job-level multi-tenancy (each customer gets their own
process) for stronger performance guarantees, cache is one of the few services
at Twitter that spins up hundreds of similar clusters in each DC. Thus, cache
operations can benefit greatly from full automation.

Pelikan makes automation easy and reliable by eliminating the performance and
resource edge cases that would require one-off tweaks. Pelikan `generator`, an
automation script set, takes advantage of the predictability in runtime
characteristics and configuration, and creates cluster profile/job/monitoring
from a simple input vector. Manual configuration-as-code check in becomes
unnecessary with this tool, and cluster creation/update is fully reproducible.

## Risk & Alternatives

We have talked about the alternatives since the beginning of this document.
Here we focus on risks.

Replacing Redis requires partial re-implementation of the rich data structures
in Redis- Redis supports 6+ data structures, and we need at least 2 to cover
Twitter’s use cases. Redis also receives broad community attention, but it
doesn't appear to accept much community influence in its core design.

Twemcache is very stable at this point. The risk of moving away is primarily
about bugs and instability. However, Pelikan has been thoroughly tested and
vetted for years, including in production, so we expect the risk to be temporary
and manageable.

## Migration Story

Pelikan aims to achieve its goals without distracting users with hands-on
migration. Protocol compatibility allows gradual and seamless transition without
changing any client-side logic.

When users do want to migrate to a different binary/protocol for new features,
e.g. from Memcached to Redis, they should be able to do so with a simple client
update without having to worry about behavioral changes. Because the backends
are built and validated as different manifests from a single framework.

## Appendix/FAQ

#### Why not modify Redis or Twemcache?

This depends on how you look at the project. The development philosophy can be
summarized as "clean slate design, pragmatic code reuse".

On one hand, by not taking the position of any existing project in our design,
we mentally free ourselves from the inclination of conforming to an existing
design for most of its decisions without realizing it, or giving preference to
a “native” abstraction over “alien” ones. A clean slate approach encourages
code-base neutral design that is driven more by what we need than what we
already have.

On the other hand, we tried introducing as little untested code as possible,
and extracted from existing solutions (mostly Twemproxy and Twemcache,
occasionally Redis) whenever possible. Reused code is mapped into the
high-level design, with necessary API changes, proper coding style
normalization, and occasional optimization. When Pelikan first became
functional, we did a simple survey and found about 50% of the LOC were
"borrowed". Since then, we have done many rounds of refactoring to assure the
ongoing integrity of the codebase as we introduce more features.

#### What’s really wrong with Redis’ memory management?

Redis doesn't write its own allocator, but instead link to external allocators-
usually jemalloc or tcmalloc. When eviction is necessary to allocate for a new
object, Redis checks a relatively small number of candidates and chooses one to
evict based on some strategy (random, TTL, etc); this process is repeated until
there is enough contiguous memory for the new object. Under no memory stress or
when having near-uniform sized objects, these allocators perform very well.
However, if a Redis instance hosts many small items and a few very large items,
allocating for a large item can very much trigger many rounds of eviction before
it can be accommodated, resulting in high latencies. Timeline used to apply
heuristics to evict larger keys more quickly, despite the fact that large items
were often retrieved a lot and much more expensive to rebuild. They do so simply
because eviction delay was unbearable. Later versions of our Redis fork used
hybrid list to work around this problem, but only for a single data structure.
Another scenario where Redis can fail us is when heterogenous item sizes lead to
the ballooning of RSS, i.e. physical memory footprint. Not managing memory
directly means Redis has no control of external memory fragmentation.

In general, it is easier and cheaper to enforce good memory hygiene by taking
direct control of this resource than trying to work around the problem with
clever higher level heuristics. And this is why many data-intensive services
opt to manage memory explicitly and directly.

#### What’s the relationship between Cuckoo Hashing and Slab Allocation?

They are swappable modules with compatible interfaces, and can be replaced with
one another depending on what properties are desirable for the workload. Cuckoo
hashing integrates indexing and data storage in a single data structure, while
Slab creates a separate hash table to take care of lookup aside from the actual
data, which are stored in slabs. For that matter, Pelikan also has a CDB module
that can use persistent storage.

All of these modules should follow compatible interfaces to simplify
development, and one can imagine mixing different protocols with all of them.
This is what we mean by "a Modular Cache". For example, mixing slab with
memcached protocol gives you Memcached/Twemcache, mixing cuckoo hashing with
memcached is what we call Slimcache. Similarly, We have started to put RESP
(Redis protocol) on top of either slab or cuckoo based on the workload.
And Pelikan makes it easy and cheap to do all of them at once.

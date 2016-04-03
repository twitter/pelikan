---
layout: post
title:  "What is caching?"
date:   2016-04-03 04:56:00 -0700
author: Yao Yue
twitter_username: thinkingfish
---

There are many different definitions of caching. And indeed caching is
[ubiquitous](https://www.quora.com/How-to-understand-Computer-Science-has-only-three-ideas-cache-hash-trash)
as long as locality can be taken advantage of. From CPU to CDN, from hardware
controllers to Wide Area Networks, caching varies greatly in its medium and
location, and as a result, its speed. SRAM-based CPU cache clocks a mere couple
of nanoseconds per operation, while downloading an image from the nearest CDN
server can take seconds. But there is one invariant – people use cache as a way
to get data faster and cheaply.

The lifeline of caching is **performance**. It is sometimes surprising to see
how often people would take the tradeoff of slightly incorrect data in exchange
of fast data access. Because of its economy, caching is also the answer to
scalability – intended as an optimization, plenty of systems will collapse if
their caches are pulled from under.

A little paranoia about things that may slow down caching is thus understandable,
once you realize the whole existence, i.e. competitive advantage, of caching
hinges squarely on it.

## Caching in Datacenters

What we care about here is caching in datacenters (this will be what we mean by
'caching' unless otherwise specified). The goal is to find the fastest, cheapest
way to data. But before we start, one needs to understand both the underlying
infrastructure and the problem.

### The Infrastructure

Datacenters are filled with servers and networks that are largely homogeneous
and centrally controlled, compared to the broader Internet. A good solution
takes such homogeneity and predictability to its advantage. Caching in
datacenters should aim for the limit of this reality – the physics of networking
fabrics and the software stacks that send, forward and receive data.

If it takes ~100μs to send a byte from end to end, a request will wait at least
~200μs to get its response. If the kernel network stack takes 15μs to process a
TCP packet, any request over TCP will have to pay that overhead on both ends.
No system can violate "the speed of light" defined by its environment, so it is
wise to choose topological placement and storage medium accordingly.

Most datacenter networks are built with Ethernet. Available network bandwidth
ranges from 1Gbps to 40Gbps at the edge, with 10Gbps increasingly becoming
mainstream. In such a setup, end-to-end latencies are usually on the order of
100μs.

On the storage side, SSDs have a seek time similar to the end-to-end network
latency, with a bandwidth comparable to Ethernet as well, somewhere between
100MB/s and 1GB/s. Spinning disks, on the other hand, are one to two orders of
magnitude slower at seek, and are thus too slow for cache. DRAM, used for main
memory, offers bandwidths on the order of 10GB/s with ~100ns in access latency,
making it way ahead of persistent media when it comes to performance.

The following figure captures the relative "closeness" of data at different
locations:
  ![data access speed]({{ "/assets/img/data_access_speed.jpg" | prepend: site.baseurl }})


The typical datacenter infrastructure has a few implications:

1. Local memory access is significantly faster than remote memory access, it
  also offers much higher throughput.
2. SSD and Ethernet are comparable both in terms of latency and throughput,
  depending on the specific products and setup. Thus a choice between the two is
  not always obvious. However, getting data remotely opens the door for scaling
  out horizontally, as the dataset can now be distributed. This probably
  explains the dominance of distributed in-memory store over SSD as cache in
  datacenters.
3. Getting remote data stored on SSD is usually slower than memory, but not
  dramatically so. For larger objects, transfer time increasingly dominates
  end-to-end performance, rendering the difference even less significant.
4. Faster communication and/or local storage medium will be game-changers. For
  example, infiniband lowers the end-to-end latency by two orders of magnitude,
  so any data storage system built on top of it sees the relationship between
  local and remote data very differently (see [Ramcloud](https://ramcloud.atlassian.net/wiki/display/RAM/RAMCloud)).
  If non-volatile memory becomes a reality in the next few years, it will
  further blur the boundary between volatile and persistent storage, forcing
  architects to rethink their storage, including caching, hierarchy.

### The Problem

Caching is a simple problem in concept. At its core are two[^1] fairly universal
functionalities:

* storing data
* accessing data

#### Storing data
To get the performance edge, cached data is overwhelmingly stored in memory.
However, as the infrastructure indicates, SSD (and NVRAM in the future) should
be considered when the right conditions are met[^2]. And because faster storage
are more expensive, any caching solution must also have good control of data
layout in its chosen media, and make efficient use of storage real estate.

#### Accessing data

There are a great deal to consider when it comes to data access. The most
important difference is whether network is involved, i.e. local versus remote.
Locally, data access is often cheaper if it happens to be in the same address
space. This gives us several "modes" of caching.


| Mode       | Over Network?   | Comm Protocol? |
| :--------- |:---------------:|:--------------:|
| in-process | No              | No             |
| local      | No              | Yes            |
| remote     | Yes             | Yes            |

There are an array of communication protocols if one has to be used, each
presenting different performance characteristics. For example, UDP generally
boasts lower overhead than TCP for remote access. Locally, one can choose to use
Unix domain socket, pipes or messaging-passing over shared memory, which are
considerably lighter-weight compared to their networking counterparts.

### Requirements

Marrying the problem at hand with underlying constraints, caching in datacenters
is usually a combination of in-process caching, local- and remote- in-memory
caching. There are a few commonalities among good caching solutions:

* deliver latency and throughput that are close to the limits of bare-metal and
  operating systems
* often use the most lightweight protocol available for the scenario
* primarily store data in memory, using persistent storage as extension
* directly manage memory and use data structures that are memory-efficient

### Hidden Requirements

So far we have been completely ignoring the operational aspect of caching. But
as anybody who tries to keep their service up knows, operations are arguably the
biggest hidden criteria of systems running in datacenters – experienced
engineers who want to sleep through the night will always choose
production-ready systems.

The exact definition of [production readiness](http://programmers.stackexchange.com/questions/61726/define-production-ready)
is still an open question. But essentially, a production-ready system is an
operations-friendly system, which offers:

* customization and optimization through configuration
* stability and predictability in runtime, adaptive to various scales
* means to monitor and debug, such as logging and statistics
* long-term maintainability and ease of development

In case you haven't noticed, the list for production-readiness is just as long
as that for functionalities. Furthermore, some of the requirements, such as
logging, may stand in the way of achieving others, such as delivering optimal
throughput and latency.

### The challenge

Weighing all the options and balancing goals at odds with each other are the
main challenge facing anybody who wants to build a good caching solution.

## Coming up...

In the next post, we will explore some design principles that allows us to
satisfy both the obvious and hidden requirements.

[^1]: cache coherency/invalidation is both important and hard, but we have established that people are often willing to sacrifice is for speed.
[^2]: [`fatcache`](https://github.com/twitter/fatcache) is our previous attempt to do so.

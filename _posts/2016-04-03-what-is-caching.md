---
layout: post
title:  "What is caching?"
date:   2016-04-03 04:56:00 -0700
author: <a href="https://twitter.com/thinkingfish">Yao Yue</a>
---

There are many different definitions of caching, depending on contexts. Caching is ubiquitous as long as locality is present – from CPU to CDN, from hardware controllers to Wide Area Networks. Caching also varies greatly in its medium and location, and as a result, its speed. CPU cache using SRAM clocks a mere couple of nanoseconds per operation, while downloading an image from the nearest CDN server can take seconds. But there is one invariant – people use cache as a way to get data faster and more cheaply.

The lifeline of caching is **performance**, the one property that justifies its existence. People routinely tolerate slightly incorrect data in exchange of getting *some* version of that quickly. Because of its economy, caching is also often the answer to scalability – intended as an optimization, plenty of services will simply stop working if their cache suddenly disappears.

A little paranoia about things that may slow down caching is thus understandable, once you realize the whole existence, a.k.a. competitive advantage, of caching systems lie squarely on it.

## Caching in a Datacenter

Caching in datacenters is the focus here (this will be what we mean by 'caching' unless otherwise specified). Making cache worthwhile in datacenters means finding the fastest, cheapest way to data. To achieve so, one needs to understand both the underlying infrastructure and the problem.

### The Infrastructure

Datacenters are filled with servers and networks that are largely homogeneous, especially compared to the broader Internet. Caching in a datacenter should take into account this particular reality – the physics of networking fabrics and the software that send, forward and receive data. If it takes about 100μs to send a byte from end to end, a request will take at least 200μs to receive a response. If the kernel network stack takes 15μs to process a TCP packet, any request over TCP will have to pay that overhead on both ends. Caching has to abide by the rule of the infrastructure, and chooses topological placement and storage medium accordingly.

Most datacenters are still using Ethernet. Current network bandwidth ranges from 1Gbps to 40Gbps at the edge, with 10Gbps increasingly becoming mainstream. In such a setup, the end-to-end latencies are often on the order of 100μs. SSDs have a seek time at about the same level, with a bandwidth somewhere between 100MB/s and 1GB/s, also comparable to Ethernet. Spinning disks, on the other hand, have a seek time one to two orders of magnitude higher, and are thus much slow for random read/write. DRAM bandwidths are on the order of 10GB/s, with an access latency of about 100ns.

The following figure captures the relative "closeness" of different data locations.
  ![data access speed](/assets/img/data_access_speed.jpg)


The typical datacenter infrastructure implies a few things:

1. Local memory access is significantly faster than remote memory access, it also offers much higher throughput.
2. SSD and Ethernet are comparable both in terms of latency and throughput, depending on the specific products and setup. Thus a choice between the two is not always obvious. However, getting data remotely opens the door to scaling out, as the data set can now be distributed. This explains the dominance of distributed in-memory store over SSD as cache.
3. Getting data stored remotely on SSD is usually slower than remote data in memory, but still on the same order of magnitude. For larger objects, transfer latencies increasingly dominate performance, rendering the difference insignificant.
4. Faster communication and/or local storage medium can be game-changers. For example, infiniband lowers the end-to-end latency by two orders of magnitude, so any systems that builds on top of it must re-evaluate the relationship between local and remote data access. If non-volatile memory becomes a reality in the next few years, it will blur the performance boundary between volatile and persistent storage, forcing architects to rethink their storage hierarchy.

### The Problem

Caching in datacenters means getting data from a location other than the canonical source, e.g. from a database or a series of computation steps. At its core are two functionalities:

* storing data
* accessing data

To get the performance edge, cached data is overwhelmingly stored in memory. However, as the infrastructure indicates, SSD (and NVRAM in the future) should be considered when the right conditions are met. From a resource point of view, any caching solution must also have good control of data layout and resource footprint, to achieve good storage efficiency.

There are a great deal to consider when it comes to data access. The most important difference is whether the network is involved, meaning local versus remote. One can also directly access local data if it happens to be in the same address space. This gives us several "modes" of caching.

| Mode       | Over Network?   | Comm Protocol? |
| :--------- |:---------------:|:--------------:|
| in-process | No              | No             |
| local      | No              | Yes            |
| remote     | Yes             | Yes            |


On top of that, there are an array of communication options, each presenting different performance characteristics. For example, UDP generally boasts lower overhead than TCP. When the data is stored locally, one can choose to use Unix domain socket, pipes or messaging over shared memory, which are considered lighter-weight compared to their networking counterparts.

### Requirements

Marrying the problem at hand with underlying constraints, caching in datacenter is usually a combination of in-process caching, local- and remote- in-memory caching. There are a few commonalities among good caching solutions:

* deliver latency and throughput that are close to the limits of bare-metal and operating systems
* often use the most lightweight protocol available for the scenario
* store most data in memory, using persistent storage as an extension
* directly manage memory and use data structures that are memory-efficient

### *Hidden* Requirements

Until now, we have completely ignored the operational aspect of caching. But operations is arguably the biggest hidden assumption about most systems running in datacenters – it has to be a production-ready system.

The exact definition of [production readiness](http://programmers.stackexchange.com/questions/61726/define-production-ready) is still an open question. In a gist, a production-ready system is an operations-friendly system, which offers:

* customization and optimization through configuration
* the ability to log meaningful events for monitoring and debugging
* statistics that reflect the state of the service
* stability and predictability in runtime characteristics, preferably at various scales
* on-going maintainability and room for new features

One may notice that the requirement list for production-readiness is even longer than the one for basic functionality! Furthermore, some of them, such as logging, may stand in the way of achieving some other goal, such as delivering optimal throughput and latency.

Balancing goals at odds with each other and weighing all the options remain the main challenge facing anybody who wants to build a good caching solution.

## Coming up...

In the next post, we will explore some design principles that allows us to satisfy both the obvious and hidden requirements.

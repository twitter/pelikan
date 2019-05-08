---
layout: post
comments: true
title:  "Memory Matters"
date:   2019-05-07 22:00:00 -0800
author: Yao Yue
tags: design, performance, operations
twitter_username: thinkingfish
---

*This is the fourth post in our [blog series](http://pelikan.io/blog/)
about the design, implementation and usage of caching in datacenters.*

Memory allocation and free don't always rank high on performance tips unless you
need to be almost always Blazingly Fast<sup>TM</sup>, which happens to be what
is expected from a cache server.

Memory management is central to in-memory caching. For a lightweight server that
does not require too much computation, memory related operations such as access,
allocation, free, and copy are some of the most expensive operations. They could
have a direct impact on throughput, but more important, they *often* have a
direct impact on tail latencies and latency variability. Another dimension of
memory management is space efficiency- because cache typically utilizes a large
amount of memory, efficient management is important to the overall resource
footprint.

## Memory Considerations

There are three aspects of memory allocation to consider:

* **Throughput**:
  * How many operations (allocate or free) can we perform per second? This
matters for overall throughput if memory allocation is needed for some or all
requests, such as allocating I/O buffers or data storage for writes.
  * What is the memory bandwidth? In most modern hardware configurations memory
bandwidth stays above network bandwidth, and most cache objects are fairly
small, rarely in the MB range. So for memory bandwidth to be a problem, the
requests usually have to trigger moving more data than the what is in the
request payload. An example would be `append` in Memcached.
* **Latency**: how long does memory allocation take? Because there's no way
around the wait due to memory allocation, it directly contributes to the latency
of any processing that calls it.
* **Space overhead**: overhead generally comes from two sources, bookkeeping for
data stored such as metadata, and wasted space that holds no data. The amount of
space spent on metadata is often obvious, while wasted space, aka fragmentation,
can be hidden. Fragmentation itself comes in two flavors: internal fragmentation
(under-utilized capacity) and external fragmentation (extra footprint).

It turns out that throughput of most memory allocators varies quite a bit
depending on the allocation size[^3], and tend to yield lower throughput for
very large or very small allocations (though a good memory allocator often
try optimizing for small allocations). Given that state-of-the-art cache
servers often promise throughput well over 100K on modern hardware, we
probably want to monitor when to invoke memory allocation if we want to
deliver that promise for more than just read.

Allocation latencies are almost always low (dozens of cycles), except
occasionally it is not, and the worst case latency is virtually unbounded.
Moving a point is cheap which is what happens most of the time under the hood,
but if one needs to ask for a new physical page, that page allocation triggers
expensive operations such as page table update and potentially some kernel
house-keeping, and this is before considering the possibility of swap.

Space overhead itself is not always a performance issue, although a larger
active memory footprint often means reduced CPU cache performance. On the other
hand, external fragmentation can lead to serious reliability issues, such as
paging or OOM.


## Optimize Memory for Cache

Memory management is crucial to cache in two ways: First, the effectiveness of
cache is often bound by the amount of data that can be fit into physical memory,
so reducing space overhead improves the economy of caching. Second,
predictability in terms of time cost of memory operations is key to achieving
consistently high performance in cache.

One thing worth mentioning is that both latency and fragmentation issues of
memory often evade performance benchmarking, which tend to have more uniform,
simplistic workload that rarely trigger expensive memory-related events or
pathological fragmentation. Therefore, users can be blindsided when such issues
pop up in production, often sporadically and seemingly for no reasons.

To minimize such penalties and surprises, a good cache should aim to guarantee
memory predictability by design, primarily via preallocation.

### Preallocation

Probably the most helpful thing one could do to avoid performance penalties from
memory operations is to eliminate such operations to the extent possible. And
the one single trick to achieve that is to allocate most, if not all of, the
memory the service instance needs in advance.

The software patterns reflecting this design decision include invoking
allocation during the explicit setup stage of the corresponding module, and the
use of resource pooling of high cardinality object types such as connections,
requests, etc. Both are well understood and often practiced patterns in writing
production software. Compared to allocating and freeing objects as demand show
up or disappear, preallocation gives us much tighter bounds both in terms of
memory size and operation cost. There are two downsides as well, one is the
additional code needed for the logic, the other is the fact that data corruption
becomes more likely if objects are reused. For cache, I would argue the tradeoff
is worth it.



## Coming up...

What's the blueprint of Pelikan? How did we break things down into modules and
how did we put these pieces together? An architecture overview is coming next.

[^1]:

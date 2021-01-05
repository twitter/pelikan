---
layout: post
comments: true
title:  "[draft] Rewriting Pelikan in Rust"
date:   2021-04-14 10:00:00 -0700
author: Brian Martin
tags: design, rust, storage, performance
twitter_username: brayniac
---

In-memory cache services, such as Twemcache and Redis, are widely used within
Twitter for a large variety of workloads. We depend on these caches being
reliable, fast, and efficient - storing potentially hundreds of millions of
small objects per instance with design throughputs ranging from thousands to
hundreds of thousands of requests per second. These requests must be served with
a predictable tail latency which is often sub-millisecond with typical timeouts
in to 50-200ms range. With such demanding performance and efficiency
requirements, C has reigned over the problem space. However, faced with the need
to add mTLS support to Pelikan and the desire to rapidly develop backends to
address very specific needs, we decided to evaluate Rust for use within Pelikan.

Rust is a modern systems programming language with an emphasis on writing
reliable and efficient software. It is particularly well-suited for use-cases
like cache, where predictable performance and memory efficiency are critical.
Its type system and ownership model allow the compiler to enforce memory-safety
and thread-safety, helping to eliminate nasty bugs that can lead to outages and
data corruption. The performance characteristics along with the safety
and productivity boosts that come with Rust made it a compelling choice.

## Pingserver

Our adventure in rewriting Pelikan in Rust began with the pingserver, an
over-engineered production-ready ASCII "PING"/"PONG" network service. By
starting with the pingserver, we were able to validate performance and prove
that we would likely be able to replace actual cache backends with Rust
implementations.

We settled on the equivalent threading architecture as the C implementation:

```
┌─────────────┐                                    
│             │                                    
│    Admin    │                                    
│             │                                    
│    :9999    │                                    
│             │                                    
└─────────────┘                                    
                                                   
┌─────────────┐                     ┌─────────────┐
│             │  ┌─────────────┐    │             │
│   Server    │  │ Established │    │             │
│             │──┤  Sessions   ├───▶│   Worker    │
│   :12321    │  └─────────────┘    │             │
│             │                     │             │
└─────────────┘                     └─────────────┘
```

We have two main listener threads, one for admin functionality such as stats,
and one that accepts new TCP connections and handle any TLS handshaking. Once
the session is established, it is passed over to the worker thread which then
handles network IO, request parsing, and response composition.

We found the Rust implementation was competitive with the C implementation in
terms of throughput and latency, giving us the confidence to proceed with
converting the Twemcache backend to Rust.

## Twemcache in Rust

The standard Pelikan Twemcache backend uses slab based storage. But with the
development of the Pelikan Segcache backend, we found that for many workloads
there was a significant benefit in being able to proactively remove expired
items. Based on the benefits seen with the development of Segcache, we decided
to focus our efforts on implementing the Rust Twemcache-compatible backend using
a Segcache storage design.

### Segcache Storage

The Rust implementation of Segcache attempts to closely follow the C
implementation. We have three main components, the hashtable, segments
containing items, and TTL buckets to enable proactive expiration. However,
unlike the C implementation, we use a "SegCache" struct to contain all of the
components and retain ownership over the data. This allows us to define our
high-level interface and keep the implementation details separate.

### Threading

In testing, we found that the Rust implementation performed well compared to
Pelikan Twemcache and single-threaded Twemcache. However, we have some cache
instances that have multi-threaded Twemcache configurations. While the Rust
implementation typically outperformed the single-threaded Twemcache + TLS, it
could not keep up handling the TLS streams compared to multi-threaded Twemcache.

As an experiment, we tried an alternative threading architecture to avoid making
the storage layer concurrent. Since the bottleneck is in the network IO and TLS
handling, we can divide the established sessions across multiple worker threads.
These worker threads can then communicate with a shared storage thread which
will own the datastructure and handle requests.

```
┌─────────────┐                         ┌─────────────┐                                             
│             │                         │             │                                             
│    Admin    │                         │             │    ┌────────────────┐                       
│             │                    ┌───▶│   Worker    │◀──▶│SPSC Queue Pair │◀──┐                   
│    :9999    │                    │    │             │    └────────────────┘   │   ┌──────────────┐
│             │                    │    │             │                         │   │              │
└─────────────┘                    │    └─────────────┘                         │   │              │
                                   │    ┌─────────────┐                         ├──▶│   Storage    │
┌─────────────┐                    │    │             │                         │   │              │
│             │  ┌─────────────┐   │    │             │    ┌────────────────┐   │   │              │
│   Server    │  │ Established │   ├───▶│   Worker    │◀──▶│SPSC Queue Pair │◀──┘   └──────────────┘
│             ├──│  Sessions   ├───┘    │             │    └────────────────┘                       
│   :12321    │  └─────────────┘        │             │                                             
│             │                         └─────────────┘                                             
└─────────────┘                                                                                     
```

The sessions are distributed across the worker threads in a round-robin fashion.
Future work could involve adding rebalancing based on worker load. Each worker
has a dedicated single-producer single-consumer (SPSC) queue pair to communicate
with the storage thread. The worker is still responsible for session handling
and request parsing, but now when a complete request is received, it passes the
parsed request and the session write buffer to the storage thread. The storage
thread handles the request and writes the response directly into the session/
write buffer and then returns the buffer back to the worker which will handle
TLS (if applicable) and the network IO to send the response back to the client.


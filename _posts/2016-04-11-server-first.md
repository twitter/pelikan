---
layout: post
comments: true
title:  "Server First"
date:   2016-04-11 02:11:00 -0700
author: Yao Yue
tags: design, performance, operations
twitter_username: thinkingfish
---

*This is the second post in our [blog series](http://pelikan.io/blog/)
about the design, implementation and usage of caching in datacenters.*

*If you don't care why we choose to work on cache server first, skip the post.*

The mode of cache – in-process, local, or remote – profoundly affects how the
specifics map out. For example, in-process caching is highly integrated into
the user runtime/architecture, and cannot be shared between different runtime
environments. On the other hand, local and remote caching generally follow
client-server patterns, allowing the two sides to decouple.

## Server First

It is not hard to see why we want to start with the server. The same server can
be used by many protocol-compatible clients, and the implementation has great
freedom in choosing its runtime and structures behind well-defined APIs. For
clients, a separate implementation needs to be provided for each language at
least.

Furthermore, writing and reasoning about server is often simpler than client.
For example, the server does not have to handle many of the failure scenarios
in a distributed environment, a luxury most clients cannot claim. Servers are
also run as standalone processes, with total control of their runtime. Client,
on the other hand, can live in a highly contentious and skewed reality, with
other threads, workloads or heavy garbage collection (GC) pauses affect
their performance and/or perception of events.

Focusing on building a "production-ready cache server" first allows us to gain
a firm footing over our design and implementation on more friendly terrain.

### Requirements for Servers

Remember the requirements listed in [caching in datacenters](http://twitter.github.io/pelikan/2016/04/03/caching-in-datacenters.html)? Here's how they translate to
servers:

* latency and throughput should approach what the hardware and OS can support,
  and remain predictable under various load and unpredictable background
  activities. As we shall see, the latter adds a lot to the requirement.
* allow a multitude of communication protocols to be used, such as TCP/IP,
  Unix domain socket, and pipes, to accommodate both local and remote modes of
  caching
* allow a multitude of data storage schemes, so users can pick what work best
  for their dataset and hardware configuration

On top of that, we also want to fulfill the requirements that make our servers
"production-ready", which means clean code structure, high-quality
configuration, logging and stats.

### For historical reasons...

Twitter's history of caching casts its own shadow on how we develop new servers.
Twitter has a large number of use cases with both Twemcache and a fork of Redis.
The operationally responsible first step is for the new server(s) to cover
existing users, and seamlessly migrate them while keeping the protocol interface
intact. Only then can we start thinking about new functionalities and other
exciting possibilities. Otherwise, the dev team risk further fragmenting the
support landscape, and stretch ourselves too thin to be productive.

## Coming up...

With concrete goals in mind, we are ready to talk about the design principles
that guide us through design implementation.

********
Overview
********

Cache Backend
=============

Cache backend is where cached data is stored. Conceptually, what it does is
extremely simple: requests are received over the network or other IO media, the
requests are then parsed, followed by processing, which mostly consists of
retrieving and/or updating data in memory, eventually, responses are formed and
sent back to the client.

In terms of functionality hierarchy, processing has a dependency on both syntax
and storage (usually in-memory). It is possible to mix and match different
protocols and storage schemes for different backends. Receiving and sending data
is delegated to functionalities provided in ccommon, including buffer
management, channel (such as TCP connections) management, and event-driven IO.

(insert a chart for cache backend dependencies)

Caching is a performance-critical piece of infrastructure. In many systems, it
is the key provisioning that allows the service to scale. Even though cached
data is logically redundant, in production the tolerance to cache misbehavior
or slowdown remains low. Cache is also stateful, meaning local issues can
easily propagate due to request fanout, and are likely to persist. Cache must be
highly reliable in terms of performance, and *statistically reliable* in terms
of data availability.

Understanding cache use cases is important for the entire caching solution.
However, it is particularly important for designing the backend. For performance
and extensibility considerations, it is better to allow requests to pass through
cache proxies with as little processing as possible. But each new use case *has*
to be supported by the backend, and will have an impact on both the protocol
syntax and storage. To understand what the cache backend is designed *for*,
please read about Typical Cache Use Cases.


Design Goals
============

The basic functionalities of a cache backend are utterly uninteresting to
anybody who has a little knowledge about what caching is supposed to provide.
And that's not why we created Pelikan. Instead, we differentiate from existing
implementations on *how* we implement these functionalities and *how well* we
achieve the following goals:

* clean, well-defined abstractions to minimize duplicated logic, through
  composable and configurable modules
* built-in observability support *everywhere*, this includes logging, stats, and
  tracing
* efficient, deterministic runtime behavior and controllable resource management

To understand why we elected these goals, please read Problems and Priorities.

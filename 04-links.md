---
layout: page
title: Links
permalink: /links/
---

#### Cache Server

* [`memcached`](http://memcached.org/): one of the earliest and arguably the
  most well-known cache server out there.
* [`redis`](http://redis.io/): a very popular, feature-rich data structure
  server that is often used as cache.
* [`twemcache`](https://github.com/twitter/twemcache/): Twitter's fork of
  memcached.
* [`fatcache`](https://github.com/twitter/fatcache/): a memcached-compatible
  cache server that runs on SSD.

#### Router / Cluster
* [`twemproxy`](https://github.com/twitter/twemproxy/): a proxy for memcached
  and redis with cluster management capabilities.
* [`mcrouter`](https://github.com/facebook/mcrouter/): a memcached protocol
  router.
* `redis` comes with native clustering support starting from [version 3.0](https://raw.githubusercontent.com/antirez/redis/3.0/00-RELEASENOTES).
  Additionally, commercial solutions are provided by [Redis Labs](https://redislabs.com/redis-cluster)

#### Client
* redis clients: [redis.io](http://redis.io/clients) has an excellent
  overview of all the popular redis clients.
* memcached clients: as numerous as the dazzling collection of redis clients.
  Here are a few:
  * [`libmemcached`](http://libmemcached.org/libMemcached.html): a C/C++ client
    library and tools for memcached.
  * [`python-memcached`](https://pypi.python.org/pypi/python-memcached): a python
    client library for memcached.
  * [`finagle-memcached`](https://github.com/twitter/finagle/tree/develop/finagle-memcached):
    Twitter's default memcached client, part of [finagle project](https://twitter.github.io/finagle/).

#### Tools
* [`rpc-perf`](https://github.com/twitter/rpc-perf): a load generator and
  benchmark tool fast enough to keep up with the cache servers. Supports
  memcached and redis protocols.

<div class="page-info">
  <p>Contact us if you want to add or edit a link on this page. </p>
</div>

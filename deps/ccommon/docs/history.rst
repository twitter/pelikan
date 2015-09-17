Genesis
=======
The Twitter Cache team (which were part of Runtime Systems, and before that, Infrastructure) started working on a fork of Memcached 1.4.4 in 2010. In 2011, with the launch of Haplo, it also took over the maintenance and improvement of Redis.

Over time, we have made significant changes to the code bases we inherited, created and open-sourced Twemcache, Twemproxy and Fatcache. The prolification of projects that are all related to managing and serving data out of memory hints a lack of common infrastructure. And indeed, the projects we have mentioned have a lot in common, especially when you examine the core mechanisms that drives the runtime and low-level utilities.

This is why we decide to work on a project called Cache Common, or ccommon in short. Instead of stretching our developement/maintenance effort thin over all these individual code bases, it makes sense to build a library that captures the commonality of these projects. We also think the commonality may very well extend beyond just in-memory caching, and can provide a toolbox of writing other high-throughput, low-latency services.

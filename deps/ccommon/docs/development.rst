****************
ccommon Overview
****************

Development
===========

Building a Cache Common library has a lot to do with the historical context that gave rise to the idea. The current open-source caching field is both sparse and crowded- most of what people use today trace back to either Memcached or Redis, maybe forked over them. On the other hand, many of the customization and improvement individuals come up with don't make their way back into the trunk version very easily, indicating an architectural problem with the original codebases.

Fundamentally, there hasn't been enough modularity in either project, or the many that derived from them, to encourage sharing and reuse. During our own, multiple attempts to create Twemperf, Twemproxy, Fatcache and Slimcache, regardless of whether we were writing a server, a proxy or a client, we had to resort to copy-and-paste, and littered our changes across the landscape.

It certainly *feels* that there was a lot in common among these projects, so we formalized it by abstracting the commonality as modules and putting them in a single library, which is ccommon, short for Cache Common.

We went through the existing code bases that have implemented some or all of the core functionalities and have been tested in production for years, synthesized them, made changes whenever necessary, to make the APIs generic enough for all the use cases we know of. Inheriting code makes it faster and more reliable to build the library; holding the APIs against known implementations allow the core library to be generic and flexible enough for our needs.

Given that multi-threaded programs are much harder to get right than their single-threaded counterpart, and sometimes incur non-trivial synchronization overhead, the priority is to get single-threading right first, with the possibility of investing in multi-threading in the future on a module-by-module basis.


Goals
=====

#. Modularized functionality with consistent, intuitive interface.
   
#. Use abstraction to allow compile-time choice from multiple implementations of the same interface, but avoid excessive, multi-layered abstraction unless absolutely necessary.

#. Production quality code that has built-in configuration, monitoring and logging support.


#. Work well on platforms emerging in the next 3-5 years.


Modules
=======



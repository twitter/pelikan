**************
Cache Overview
**************

What is Caching
===============

What is the essense of caching?

There are many different definitions given for their more or less narrowly defined contexts. Caching is ubiquitous- from CPU to CDN, from hardware controllers to wide-area networks. Caching also varies greatly in its medium and location, and as a result, its speed- CPU cache using SRAM clocks a mere couple of nanoseconds per operation, while the nearest CDN server for a website across the globe takes seconds before sending an high resolution image. But there is one invariant- people use cache as a way to get data faster and more cheaply.

The lifeline of caching is **performance**, the one property that justifies its existence. People routinely tolerate slightly incorrect or stale data so that *some* version of the data can be served to them quickly. Caching is also the answer to a lot of scalability problems- although often labeled as an optimization, there are plenty of services that will stop working at all if caching is suddenly removed.


Caching in Datacenters
======================

Caching in datacenters is the primary concern of this project (and we'll refer to it simply as 'caching'). How do we make caching fast among a large number of servers interconnected in a predefined networking topology?


Infrastructure
--------------

Caching in a datacenter has to abide by the rules of that particular environment- the physics of networking fabrics and how fast they can transmit data, and the software that regulates the sending, forwarding and receiving of data. If it takes at least 100μs for any data from server A to reach server B and vice versa, it will take at least 200μs to request a chunk of data and receive it. if the Linux kernel networking stack takes 15μs to process a TCP packet, more time will be added to fulfill each request over TCP. Fast caching means getting on the best environment available- a faster networking infrastructure, or the choice of a faster medium (local memory over remote memory, memory over disk) can have a huge impact.

Most datacenters are still based on ethernet. Network bandwidth ranges from 1Gbps to 40Gbps, with 10Gbps increasingly becoming the common option. The end-to-end latencies between servers are often on the order of 100μs. SSDs have a seek time at about the same level, with a bandwidth somewhere between 100MB/s and 1GB/s, also comparable. Spinning disks, on the other hand, have a seek time that's 1-2 orders of magnitude higher and thus much slow for random read/write. Main memory (DRAM) bandwidth are on the order of 10GB/s with an access latency at about 100ns, with a fast increasing capacity per unit cost. The following figure captures the relative "closeness" of different data locations.

.. image:: _static/img/data_access_speed.jpeg

The infrastucture commonly available implies a few things: first, local memory access is significantly faster than remote memory access, it also offers much higher throughput; second, SSD and LAN performance are comparable both on latency and throughput, depending on specific products/setup, indicating making a choice between the two is not trivial, either. However, getting data remotely means the system will have better scalability w.r.t. both data size and bandwidth through sharding, which may explain the dominance of distributed caches. Finally, it is worth mentioning some game-changing technologies: Infiniband lowers the E2E latency by two orders of magnitude, and often completely demands re-architecting  the systems built on top of it. Emerging medium such as nonvolatile memory further blurs the boundary between various storage media, and will require architects to rethink their storage hierarchy.


Design
------

How do we approach caching from a design perspective?

Assumptions
^^^^^^^^^^^

The reality of infrastructure today means a few design decisions are common:

#. "hot" data usually reside in main memory and sometimes in SSD, but if such data also comprise mostly of small objects (by standards of SSD page size) without locality among them, then they almost always reside in memory, because SSD is not efficient for tiny reads/writes. Larger data can be more efficiently stored with SSD while still keeping up with ethernet.
#. given data size and scalability requirement, cache is managed as a stateful distributed system, and sharding/routing is required. Given the popularity of key-value store and NoSQL databases, caching often takes the format of distributed, in-memory key-value storei and applies sharding applies to key only. In many cases, cache even looks and behaves like a database.
#. local caches, ones that can be visited by inter- or intra-process communication, are used when the network bandiwdth and/or latency becomes a bottleneck, especially when they create unevenness or "hotspots" among the data shards.


On top of these decisions, efficient caching must continue to manage the infrastructure real estate well by doing as little non-essential work as possible, and having as little interference as possible.


Layered Functionality
^^^^^^^^^^^^^^^^^^^^^

It is helpful to learn from existing distributed systems that are stateful and performance oriented, one of them being the networks themselves. Also having handle states while trying to maximize throughput and minimize latencies, networking technologies in recent decades adheres to the divide between control plane and data plane (aka forwarding plane) rather strictly. In short, control plane is the part of the logic in each node that deals with the state of the distributed system- how each node should interact with other nodes properly; it also hands useful, state-related information over to data plane so the latter can perform logic such as routing correctly. On the other hand, data plane is where each individual request or data exchange is handled, this is where the performance is perceived by the end user. So it is not surprising that data and control plane are responsible for carrying out work on the "fast path" and "slow path" correspondingly, and a trip through the control plane is meant to be slower. Networking community demonstrated that by having clear divide between different parts of the systems that are optimized for different goals, they can make packet processing fast while keeping the state of the system well-managed.

The claim here is that we should apply the same analogy to high-performance caching systems. The layered networking model is an effective way of minimizing work and interference on performance-critical paths. By explicitly calling out functionalities and components that are performance critical or not, and segregate them as much as possible, we can better define the work that *has* to happen for every single request, while pushing other functionalities elsewhere. Furthermore, we can take measures so that the "fast path" operations take priority, and are not interrupted or interfered unnecessarily by those happening on the "slow path". Both kinds of mistakes are especially likely or even tempting when functions, threads, and processes *can* run indiscrimintively on the same set of system resources, seem perfectly blendable under the name of code sharing and reuse, unlike many dedicated routers where even hardware is highly specialized for the particular plane it serves.

Software-defined networking brings distributed storage systems and networking systems even closer together in the datacenter. Traditionally, control plane decisions are often made independently by each node, since large-scale communication is unpredictable or plainly impossible. However, datacenters represent a special kind of environment where homogeneity and centralized control can be achieved much more easily. Reaching concensus in a large distributed system is expensive and slow [citation needed], so most scalable solutions delegate the decision-making to a central location, represented by one or a relatively small number of nodes. Orchestration increasingly applies to both network topology as well as sharding, and a lot of the functionalities at the control plane level are increasingly drained, centralized, and the control plane inside each node greatly thinned. This emphasizes the importance of an often-neglected term "management plane" [#]_, which is the centralized brain of distributed systems, and serves as the interface where human operators will come to interact with an otherwise highly abstracted and automated [#]_.

Organizing functionalities into layers is more than a frivolous exercise. It provides a powerful mental model to focus and differentiate. For example, once we establish a boundary between data and control plane, it becomes more natural to make different language choices for different parts- we may want to use a highly expressive, and potentially verifiable language/implementation for the control plane, while leaning toward languages that are closer to bare-metal hardware for the data plane. The management plane, due to the need to interact with operators, may call for yet another language that's declarative in nature. We thus match each plane with languages that enhances the most desirable properties for that particular layer. Similar considerations can be found throughout the design process, where such a division can be liberating.

Anatomy
^^^^^^^

There are four roles in a caching system: server, client, proxy, and manager. Servers collectively hold all the data, decide data retention policy, apply updates, and serve other requests related to data. Clients initiate the data requests. Proxies route and dispatch data requests, either by sending it to a server, or by sending it to another proxy. Manager determines the topology and routing policies which proxies follow, and may also monitor the health of servers and other roles if necessary.

We are calling these entities roles instead of parts or nodes because they are logical. While these roles often have their separate modular representation, they don't have to be "physically" (i.e. machine-wise) separated. The proxy can run along side the client, or the server, or by itself. All three entities may reside on the same machine, the proxy may degenerate and disappear when routing is static and simple, etc. However, the functionalities these roles provide are universal in any caching system. For example, finagle-memcached as a library serves as a combination of the client role and proxy role. Many memcached users using such a client also skip an explicit manager, but assume server topology is mostly fixed, and requires human intervention when a server is offline, thus effectively turning the system operator into a manager. When a cache is in-process, neither proxy nor manager is necessary, since routing is trivial and the availability of the cache is guaranteed as long as the process is alive.

One of the simplest computing model in a distributed system is the client-server model, and that's how caching started. Here, we call out an more complex four role model mostly based to two facts. First, caching systems are stateful since they hold a large amount of data, this means having a single functional view of the system topology is crucial to route requests correctly and consistently (i.e. clients won't diverge on their world-view). To reach a concensus among a large number of nodes would be difficult and expensive, if not impossible. And even the task of monitoring the topology is unnecessarily complicated for individual nodes. This justifies the role of the manager. Second, caching is rather prevelant in modern Web architecture and other types of data-intensive applications. With the increased popularity of microservices, many components in a single system will have their own needs for caching, which often can be served using the same technological stack but individual configurations. While functionalities such as routing is fundamental to the service, it can be involve a fair amount of computation, and s often subject to change. Hence it quickly becomes a logistic nightmare trying to coordinate with dozens of different clients, which in turn means dozens of customers/teams, to apply any nontrivial updates. This practical concern drives owners of the caching technology to minimize the interface visible and managed by their customers- in other words, a thin client that doesn't know or worry about state of the whole system. This preference thus justifies proxy as its own role, so routing and other features can be provided outside of the customers' direct control.

The different functionality layers and roles will be discussed in more details in their own section.


.. [#] `Remembering The Management Plane <http://networkheresy.com/2012/09/15/remembering-the-management-plane/>`_

.. [#] `The Control Plane, Data Plane and Forwarding Plane in Networks <http://networkstatic.net/the-control-plane-data-plane-and-forwarding-plane-in-networks/>`_


*******
Anatomy
*******

We take a high-level look at the main functionalities in a caching system and how they are performed by various roles.

Life of A Request
=================

A cache request is rather simple- a request is made for some identifiable data, and a result is returned regarding the state of that request.

Request Remote Data
-------------------

Local Proxy
^^^^^^^^^^^

A request starts with the client, which has to serialize it according to some agreed-upon protocol to prepare for network transmission. If the proxy is co-locating with the client, the request is then examined by the proxy, which picks out the useful bits for routing (e.g. the key) and comes up with one or more destinations. The encoded request is then sent over the Ethernet and (hopefully) reaches one or more servers. Each server, upon receiving data from the network, de-serializes the request, processes it, and sends back a reply in the same route that it came.


Remote Proxy
^^^^^^^^^^^^

If proxies are run as independent processes on separate machines, additional work needs to be done before routing or other tasks. Proxies have to both receive and send data from the network, perform the same serialization required for clients and de-serialization required for servers. Proxies can also translate between different protocols, or intercept replies and alter them.


Server-side Proxy
^^^^^^^^^^^^^^^^^

If proxies are colocated with servers, they will mostly act as a remote proxy except when the routing destination is the same as their current process, in which case it should act as a server.


Failure Scenarios
^^^^^^^^^^^^^^^^^

The failures of clients are out of scope of the cache design (although, failing to complete a write may introduce data inconsistency or corruption down the line, so that should be a concern for users). A failure in the proxy, such as failing to forward a request, can be effectively mitigated by a combination of client retry and reroute to a different proxy. Individual requests may still fail after exhausting the common options, but these failures should be viewed as independent, rare events without further ramificationsand can be safely retried at the client level.

The failures of servers are much more serious. A server going away means the topology is temporarily wrong. This not only means further retry without topology update is unlikely to help, but also means any future request destined to this particular shard will probably also fail until the topology is somehow "fixed". Unlike proxy failures, the best strategy here is not to retry, even reroute might be possible in some systems for certain type of requests, but to wait until topology is updated. It is very easy for a large cohort of clients to DDoS a single server through some aggressive retry logic that stateless systems with many nodes can much more easily survive. However, topology updates are not part of the "normal" request path, and thus don't belong to the fast path. It happens a lot slower compared to request rates and can affect many requests in a row.

When the various roles make logical mistakes instead of flat out fall off the map, the situation gets more complicated. For example, incorrect routing decision at the proxy role may lead to data inconsistency, which is both subtler and more difficult to deal with than servers going offline. A lot of similar mistakes effectively corrupt the topology, which requires detection of such issues in a caching system. Unfortunately, unlike many databases, such logic is often missing in caching, trading correctness for performance. Because of such risks, users should never keep their data in cache indefinitely. A much safer practice is to always let data expire after a certain period of time to bound the amount of inconsistency that may persist.

Local Cache
-----------

When the cache is local, it often means the proxy role degenerates to a statically configured target for the cache. The communication protocol is also switched to something cheaper and simpler compared to ones used for RPC. The client and server roles look largely unchanged.

In-process Cache
----------------

When the cache is in-process, the boundary between client and server also melts away. There often isn't any explicit communication protocol at all, as the most efficient thing to do is to pass memory references. Similarly, serialization is short-circuited by in-memory format. In-process cache becomes a packaged version of one of more data structures.

Importance
----------

The data plane is defined by the functionalities needed during life of a request, and nothing more.


Life of A Server
================

Servers *are* the topology. With a correct set of servers, there are numerous ways to reliably come up with a configuration that would work. Proxies and clients also need to be aware of the topology, but their stateless status w.r.t. cache data means it is far simpler to manage them.

What happens when the server set changes? When a server first joins an existing topology, it needs to make its presence known by signaling the manager or making the information descoverable. The manager re-evaluates the topology with a new set, and distributes the new topology or the update to all proxies. Here the catch is that the update may be done in a centralized, or consensual way by the manager, but the distribution is probably asynchronous in nature, and thus proxies can fall out of sync with each other. Most existing systems simply ignore this scenario and deem the "gap" small enough to be overall safe. When a server goes offline, it can be graceful- where the server tells the manager, or sudden- where there is nobody there to announce the departure. In the latter case, the manager needs failure detection to catch the quiet quitters. Either way the manager then proceeds with topology change and announcement.

Managing server membership and status is undoubtly the core of the management/control plane, while the distribution of such information another important, but often overlooked aspect.


Manager
=======

There isn't a life cycle expected for the manager. As a role it is supposed to be there for all the decisions that need to be made about system topology and policy, and monitor the whole system at a high level. When the manager goes away, topology is temporarily frozen and the system becomes vulnerable against future topological changes. A breakdown in the manager often requires human attention and fixes that are not built into the system design. As much as software developers would like to automate everything, the manager role stands as the last frontier between human operator and the system itself.

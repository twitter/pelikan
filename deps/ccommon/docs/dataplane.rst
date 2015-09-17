**********
Data Plane
**********

Data plane is conceptually the simplest, but with the most stringent performance requirements. As such, it often favors simple, deterministic algorithms and data structures, and an efficient, deterministic runtime.

All caching scenarios except for the in-process cache requires inter-process communication between the client and server (by ways of proxy as needed), and the successful processing of requests in these scenarios is not guaranteed, even when the other processes are local to the machine. Depending on the setup, the system may resort to different media and protocols to carry out the communication. If the requests are sent to different machines on the network, most likely TCP connections or UDP will be deployed. When the destination is local, more efficient communication can be achieved over other media such as Unix Domain Socket, pipes, or shared memory. At the application level, the requests need to be packed and unpacked for the communication as well, using protocols defined by specific solutions. Memcached has both an ASCII and a binary protocol that have much syntatic overlap but are lexically distinct. Redis uses RESP (REdis Serialization Protocol) between client and server, which is a plaintext protocol incompatible with either of the memcached protocols. Twitter has been using Thrift partially along the communication path of Nighthawk.

Overall, choosing a communication protocol right for the scenario and having an efficient, easy-to-understand application protocol is rather crucial to data plane performance, since communication dominates clock cycles and resources in the simple use cases [#]_.

The programming model around inter-process communication is a subject of great complexity and headache. Generally it is agreed that a synchronous model that provide concurrency by using kernel threads does not perform or scale well [citation needed], unless support for user-level threads are provided [citation needed]. Asynchronous programming allows us to scale better by multiplexing many communication channels in each running thread, avoiding idle waiting, and keeping thread-related overhead under control. However, the burden of keep states now falls on application developers. The apparatus for asynchronous communication- event libraries and asynchronous IO syscalls, are not most programmers' friend, and development can be slow and buggy. And many choose to use an abstraction that hides the implementation details, such as `Finagle/Netty <https://github.com/twitter/finagle/>`__.

It is quite obvious that dealing with data inside a process's own address space is significantly easier than remote data. And thus it makes sense to draw a line there- application logic handles computing based on data already in memory, while a library takes care of everything else.

Cache Common *is* the library that "takes care of everything else".

.. [#] `Twemcache Performance Analysis <https://github.com/twitter/twemcache/wiki/Impact-of-Lock-Contention>`_

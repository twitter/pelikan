*************************
Twitter's Cache Use Cases
*************************

Here we assemble a list of typical and important cache use cases at Twitter. The goal is to offer some context on how people are using cache, so we can provide continuous support and development to improve these use cases. The entries here should be viewed as important categories instead of an exhaustive list.


Flat Values
===========

Simple Blobs
------------

Description
^^^^^^^^^^^

A simple key whose value is a binary "blob". This works for cases where the value is almost always used as a whole, such as a string, or is small enough that retrieval overhead is low. This is the oldest data model of key-value caching.

Examples
^^^^^^^^

* gizmoduck (user service) caches user object in cache
* talon (t.co URL service) caches shortened URL that points to original URL


Simple Counts
-------------

Description
^^^^^^^^^^^

A simple key whose value is a number (currently an integer due to implementation constraints in most backends). These can be backed by persistent store, or not. Noteworthy API usage includes `incr` and `decr`. Since `incr/decr` are not idempotent, the counts can be slightly off compared to the true value. But it is often good enough when exact value doesn't matter too much.

Examples
^^^^^^^^

* limiter service uses cache for per-source or per-endpoint rate limiting. Not backed by DB.
* tweetypie (tweet service) stores reply/fav/retweet as simple counts. Backed by DB.

Structured Values
=================

Attribute Sets
--------------

Description
^^^^^^^^^^^
A primary key whose value is a record, i.e. a collection of attributes. Most data objects Twitter deals with are reasonably rich, e.g. user objects and tweet objects, but it may not always be worthwhile to treat them as such in the cache backend. Whether they fit better into the unstructured or structured key schema depending on (at least) the following factors:

#. size of the object value: keys with a few attributes and small values can often be more efficiently packed as a simple string as compared to structured keys, object with a lot of attributes often can be updated more efficiently with structured keys
#. extensibility of the attribute fields (variability): if a lot of the attributes are optional, it is easier to query a small subset of them with structured keys
#. access pattern: if all attributes are always used together (e.g. tweet hydration), structured keys don't provide much value


Timelines
---------

Description
^^^^^^^^^^^

Indices of content. Most of Twitter's public content are organized this way. For example, a user timeline which contains all tweet IDs created by a particular user and sorted in reverse chronological order is the building block of many other views, such as home timeline. The timeline entries are often homogeneous and tiny- integers or other unique identifier types. The natural data structure for this use case is sorted set or list, with flexible sorting criteria for timelines of different nature (by timestamp, by local key, by weight/tag...). Updates are almost always incremental and small, but account for the majority of steady-state throughput due to fanout. This use case really benefits from data structure support, as read-modify-write greatly increases bandwidth consumption and write contention. Updates mostly happen at one end of the timeline- fanout appends to the newest end, and truncation happens on the oldest end; insertion and deletion at arbitrary location are relatively infrequent.

Example
^^^^^^^
* timeline service which has over a dozen timelines of different nature, almost all cached

Time series
-----------

Description
^^^^^^^^^^^
Compared to timelines, time series have values associated with each entry in the sequence, instead of the entry being the only content that matters. Most of observability metrics fit into this type, where the metric name remains the same over a long period of time and series is made up of values gathered at different timestamps. Most of analytical data processed via streaming compute pipelines also fit. The natural data structure for this type is also sorted set, and almost always sorted by local key. The update pattern and read/write ratio appear very similar to timeline, but the update operation often involves arithmetic operations such as incr/decr. So this is equivalent to a simple counter use case but with structured keys.

Example
^^^^^^^

* metrics and values which are recorded periodically
* data analytics maintains per-time-window tweet impression and engagement counts for target


Metrics
=======

The metrics module provides a light-weight library to declare, update and report metrics.


Metrics can be turned on or off at compile time. They can also be enabled, disabled and composed at the module level. For example, if an application uses both the ``event`` module and ``tcp`` module, it is perfectly fine to disable metrics on the ``event`` module while enabling it for the ``tcp`` module. You only pay for what you use.

The desire to compose metric groups drives a lot of the design decisions of metrics, as we shall explain below.

Background
----------

If good abstraction works when it hides as much unnecessary details from users as possible, then good observability plays the complimentary role. Instead of enclosing functionalities in a black box, it offers a glimpse into their internal state and structure. If everything works perfectly, abstraction is all that is needed. However, when things go wrong, as they inevitable do eventually, observability is the first thing developers turn to for monitoring and debugging.

Good observability is one of the defining properties of production systems, as compared to prototypes and toy systems which often lack it. But because it is orthogonal to the functionalities of the system being built, and often not included in the initial design and implementation, a lot of systems end up having observability that feels bolted on and alien.

Goals
-----

Through practice, we found there are a few properties that we really appreciate:

#. metrics should be cheap to update and retrieve, so developers don't have to worry excessively about overhead and performance degradation caused by observability.
#. there needs to be an easy way to provide and look up the meaning of every metric, so readings can readily translate into understanding and insight.
#. for reusable modules or libraries, having metrics reported the same way everywhere is highly desirable, so developers know what to look for and can rely on their experiences from elsewhere.

In reality, achieving all these goals at once is often a luxury. For example, many libraries offer no visibility by themselves, forcing developers to create and maintain metrics above the library APIs. This means what's visible is limited by API exposure, which is often less than what the internal states capture. It also means duplicate work among different users. When libraries do provide metrics, they are offered as standalone and cannot be easily combined.

Given the scope of this project, we not only have control over how functionalities are implemented, but also can influence the desirable way of organizing and using these functionalities. In other words, we can afford to dictate how we think observability should be provided. With this assumption, it turns out we can find a way to achieve all these goals at once.

Design
------

Metrics are part of each module. Almost every module comes with a predefined group of metrics that capture its state and usage. This saves developers the trouble of managing metrics at the application level and minimizes duplicate work among similar use cases. Users can follow the same convention to group and define metrics outside of this library, and mix everything together.

A metric groups takes on the dual format of a structure whose members are metrics, and an array whose elements are metrics. This is made possible with the use of C preprocessors, and makes both updating (lookup by name, via the structure interpretation) and reporting (traverse by iteration, via the array interpretation) efficient. Users can further create a global structure containing multiple metrics groups, whiles still using the same flattened array view to process all metrics.

Metrics should be allocated globally, and the address for each module's metric group(s) should be provided during module setup (or ``NULL`` to turn off metrics). This means users have total control of the memory layout of metrics, at the cost of having to setup module up with such information. In practice, this often means global metric reporting is extremely straightforward.

Metrics are updated and reported using atomic instructions, not locks. When combined with zero- or low-contention threading model, metric access is extremely efficient.

Data Structure
--------------

.. code-block:: C

  typedef enum metric_type {
      METRIC_COUNTER,
      METRIC_GAUGE,
      METRIC_FPN
  } metric_type_e;

  struct metric {
      char *name;
      char *desc;
      metric_type_e type;
      union {
          uint64_t    counter;
          int64_t     gauge;
          double      fpn;
      };
  };

``metric`` is the fundamental data structure, each metric has (for now) three types, a printable name, a short description, and value.

If a metric is of type ``METRIC_COUNTER``, its value always increases monotonically. A metric of type ``METRIC_GAUGE`` has a signed integer value. Type ``METRIC_FPN`` means the value is a floating-point number.


Usage
-----

Declare and initialize
^^^^^^^^^^^^^^^^^^^^^^
.. code-block:: C

  METRIC_DECLARE(_name, _type, _description)
  METRIC_INIT(_name, _type, _description)
  METRIC_NAME(_name, _type, _description)

To use these macros, ``_name`` *must* be a legal identifier [C11]_. See ``cc_metric.h`` for related implementation details.

A C preprocessor convention allows the above macros to be applied against a "list" of metrics. For example, one can define request related metrics, ``REQUEST_METRIC``, as such:

.. code-block:: C

  #define REQUEST_METRIC(ACTION)                                          \
      ACTION( request_free,       METRIC_GAUGE,   "# free req in pool"   )\
      ACTION( request_borrow,     METRIC_COUNTER, "# reqs borrowed"      )\
      ACTION( request_return,     METRIC_COUNTER, "# reqs returned"      )\
      ACTION( request_create,     METRIC_COUNTER, "# reqs created"       )\
      ACTION( request_destroy,    METRIC_COUNTER, "# reqs destroyed"     )

A metric group for the request module can be defined by using the ``METRIC_DECLARE`` macro against the list above:

.. code-block:: C

  typedef struct {
      REQUEST_METRIC(METRIC_DECLARE)
  } request_metrics_st;

And define a new macro to initialize the metric group with ``METRIC_INIT``:

.. code-block:: C

  #define REQUEST_METRIC_INIT(_metrics) do {                              \
      *(_metrics) = (request_metrics_st) { REQUEST_METRIC(METRIC_INIT) }; \
  } while(0)

Helper functions
^^^^^^^^^^^^^^^^
.. code-block:: C

  void metric_reset(struct metric sarr[], unsigned int nmetric);
  size_t metric_print(char *buf, size_t nbuf, struct metric *m);

``metric_reset`` resets the values of an array of metrics.
``metric_print`` prints the name and value of a metric, in human readable format, to buffer ``buf``, with a single space separating the two fields. This simple style is compatible with how Memcached currently reports metrics ([Memcached]_). Helper functions for other formats (e.g. Redis [Redis]_, StatsD [StatsD]_) may be introduced in the future.


Update
^^^^^^
.. code-block:: C

  INCR(_base, _metric)
  INCR_N(_base, _metric, _delta)
  DECR(_base, _metric)
  DECR_N(_base, _metric, _delta)
  UPDATE_VAL(_base, _metric, _val)

The ``_base`` field reflects the starting address of the metric group. Therefore, if ``request_metrics`` is of type ``request_metrics_st *``, we can use it and the metric name, e.g. ``request_free`` as listed in ``REQUEST_METRIC`` to update the metric value:

.. code-block:: C

  DECR(request_metrics, request_free);

``UPDATE_VAL`` applies to all three metric types. ``INCR_N`` and ``INCR``, which is short for ``INCR_N(_, _, 1)``, apply to both counters and gauges. ``DECR_N`` and ``DECR`` apply to gauges only.

Report
^^^^^^

Often, reporting metrics means iterating through and read/print them all. This is when the array view of metrics becomes handy.

The object that the aforementioned ``request_metrics`` points to has the same memory layout as an array of ``struct metric``. We only need to know the size of this array to traverse it, which we can get via the following macro:

.. code-block:: C

  METRIC_CARDINALITY(_o)

.. code-block:: C
  #define METRIC_CARDINALITY(_o)

Which helps us to loop through all request related metrics:

.. code-block:: C

  size_t n = METRIC_CARDINALITY(*request_metrics);
  struct metric *metric_array = (struct metric *)request_metrics;
  for (size_t i = 0; i < n; ++i) {
      /* do something with metric_array[i] */
  }


Hierarchical composition
^^^^^^^^^^^^^^^^^^^^^^^^

A full-fledged application uses many modules. Similarly, metric groups can be further assembled to provide observability of the entire service:

.. code-block:: c

  struct app_stats {
      request_metrics_st      request_metrics;
      response_metrics_st     response_metrics;
      storage_metrics_st      storage_metrics;
  } app_stats;

To work with this setup, individual modules should be initialized with the correct base address of their metric group, e.g. ``&app_stats.storage_metrics`` for the storage module. Reporting multiple metric groups works almost exactly the same as a single metric group.

Compile-time switch
^^^^^^^^^^^^^^^^^^^

All macros can be turned to no-op by turning off ``HAVE_STATS`` at compile time, which in turn undefines ``CC_STATS``.

.. code-block:: bash

  # assuming the following is issued in the build directory under project root
  cmake -DHAVE_STATS=off .. # this turns stats off globally, undefines CC_STATS
  cmake -DHAVE_STATS=on ..  # this turns stats on globally, defines CC_STATS

References
----------
.. [C11] `C11 standard <http://www.open-std.org/jtc1/sc22/wg14/www/standards.html#9899>`_
.. [Memcached] `Memcached stats command <https://github.com/memcached/memcached/blob/master/doc/protocol.txt#L496>`_
.. [Redis] `Redis INFO command <http://www.redis.io/commands/info>`_
.. [StatsD] `StatsD line format <https://github.com/etsy/statsd#usage>`_

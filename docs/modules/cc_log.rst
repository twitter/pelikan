Log
===

The log module provides a pause-less logging utility that incurs very little latency, and works well on latency-sensitive paths. It prioritizes performance over reliability, and is suitable for debug logging, event logging but not transaction logging.

Background
----------

Logging is the essential in recording important events, abnormal behavior and data samples. Unfortunately, for in-memory cache or other latency-sensitive applications, logging often presents a dilemma for developers. The naive log implementation usually writes directly to a file, an operation that is most likely buffered, but not guaranteed to return immediately. In production, it is not unheard of for a background task that nobody knows about to create contention around disk I/O, and to cause "mysterious" slowdown among mission critical services. As a result, developers often invoke logging somewhat hesitantly, and shy away from high-frequency logging practice such as request recording, fearing the negative impact on core performance.

Design
------

The nature of I/O operations against persistent media means there is always a trade-off between having data stored reliably, and introducing overhead to performance critical processing. For the purpose we are considering here- debug logging, data sampling, etc- we think it is acceptable to give up a little bit of reliability for predictable performance.

The trade-off is made with the consistent use of memory as an intermediate logging destination, a ring buffer that supports a single producer and a single consumer to be exact. A thread on the performance-critical path writes to the ring buffer which always returns immediately. Users are supposed to set up another, asynchronous (background) thread to flush data in the ring buffer from memory to their final location, which is a file. When the buffer becomes full, logged data are dropped until the next flush succeeds.

The rationale for this design comes from a few observations in production: first, when everything is working as expected, the producer/consumer logging setup is actually quite similar to buffered write. Configurable buffer size and flush interval give user some control, and I/O operations are generally more efficient with a larger chunk of contiguous data than many smaller ones, reducing the overall contention at storage devices. Second, when the system is under high load, it is usually a sensible decision to sacrifice log fidelity for maximum application throughput. Finally, if an error condition is triggering abundant exception logging that overwhelms the buffer, it is very likely for the dropped logs to be similar to existing ones. Therefore, we can afford significant data loss/drop without losing much insight into the problem. The last condition is also helped if high-performance metrics are used to provide robust statistics.

We have retained the ability to log directly to a file with the right configuration options, which are explained below. We recognize that it is nice to have a straightforward way of logging that does not require much setup, e.g. a background thread, for development or more relaxed scenarios.


Data Structure
--------------
.. code-block:: C

  struct log_metrics_st;

  struct logger {
      char *name;
      int  fd;
      struct rbuf *buf;
  };

``log_metrics_st`` declares metrics native to the ``log`` module.

A ``logger`` has three fields: ``name`` points to the name of the log file, which may be ``NULL`` e.g. if using standard outputs; ``fd`` is the file descriptor for the log file; ``buf`` points to the ring buffer used for temporary log storage, buffering is disabled if ``buf`` is set to ``NULL``.

Synopsis
--------
.. code-block:: C

  void log_setup(log_metrics_st *metrics);
  void log_teardown(void);

  struct logger *log_create(char *filename, uint32_t buf_cap);
  void log_destroy(struct logger **logger);

  void _log_fd(int fd, const char *fmt, ...);
  #define log_stderr(...) _log_fd(STDERR_FILENO, __VA_ARGS__)
  #define log_stdout(...) _log_fd(STDOUT_FILENO, __VA_ARGS__)
  bool log_write(struct logger *logger, char *buf, uint32_t len);

  void log_flush(struct logger *logger);

  rstatus_i log_reopen(struct logger *logger, char *target);

Description
-----------

Setup and teardown
^^^^^^^^^^^^^^^^^^
.. code-block:: C

  void log_setup(log_metrics_st *metrics);
  void log_teardown(void);

No magic here, just setup the metrics.

Create and destroy
^^^^^^^^^^^^^^^^^^
.. code-block:: C

  struct logger *log_create(char *filename, uint32_t buf_cap);
  void log_destroy(struct logger **logger);

``log_create`` returns a logger with the information given or ``NULL`` if an error has occurred. If ``filename`` is not ``NULL``, it will attempt to open the file with flags ``O_WRONLY | O_APPEND | O_CREAT`` and masks ``0644``. Otherwise, it defaults the output to ``stderr``. ``log_create`` uses ``buf_cap`` in creating the ring buffer. A buffer of capacity ``buf_cap`` is allocated upon successful return. However, if ``cap_buf`` equals ``0``, buffering is turned off and ``write`` syscall will be used directly.

``log_destroy`` will flush to the log file and release all memory resources whenever applicable. Note that the argument is of type ``struct logger **`` to avoid dangling pointers.

Write to logger
^^^^^^^^^^^^^^^
.. code-block:: C

  void _log_fd(int fd, const char *fmt, ...);
  #define log_stderr(...) _log_fd(STDERR_FILENO, __VA_ARGS__)
  #define log_stdout(...) _log_fd(STDOUT_FILENO, __VA_ARGS__)
  bool log_write(struct logger *logger, char *buf, uint32_t len);

``log_stderr`` and ``log_stdout`` are two convenience wrappers that make it easy to log to standard outputs. The arguments follow the same convention as in ``printf``.

``log_write`` takes a formatted string of length ``len`` stored in ``buf``, and logs it according to the way ``logger`` is created. If buffering is enabled, data is copied to the ring buffer. If the ring buffer does not have enough free capacity for the log, the entire log is skipped. Without buffering, ``log_write`` writes directly to the ``fd`` it is setup with in a best-effort fashion.

Flush to file
^^^^^^^^^^^^^
.. code-block:: C

  size_t log_flush(struct logger *logger);

``log_flush`` writes as much data to the log file as possible, and updates the (read) marker in the ring buffer. Data that cannot be written to the file will be kept until next call. If the ring buffer or the file was never setup, no action is taken. Return the number of bytes flushed.


Log reopen
^^^^^^^^^^
.. code-block:: C

  rstatus_i log_reopen(struct logger *logger, char *target);

``log_reopen`` reopens the log file according to ``name``, and does nothing if standard outputs are used. It returns ``CC_OK`` for success or ``CC_ERROR`` if reopen failed (at which point ``logger`` will no longer have a valid ``fd``). If ``target`` is specified function will rename original log file to the provided target filename and reopen the log file.

This function can be used to reopen the log file when an exception has happened, or another party such as ``logrotate`` instructs the application to do so. Log rotation in a ``nocopytruncate`` manner- i.e. the content in the file is not copied, but the file is simply renamed- is more efficient in high-load systems. But doing so requires signaling the application to reopen the log file after renaming. This function makes it possible to achieve that when used with proper signal handling.

Thread-safety
-------------
The logger is not thread-safe in the general sense. However, it is safe to use one thread as the producer, which writes to the logger, while using another thread as the consumer, which flushes the logger. A typical setup would have a worker thread being the producer and a background maintenance thread as the consumer.

If ``log_reopen`` is used with a signal, it might invalidate the previous file descriptor in the middle of ``log_flush`` execution, regardless of thread model. The impact of this is to see an exception in ``write`` and failure in clearing up the ring buffer. But as long as ``log_flush`` is scheduled periodically, it is not fatal. To avoid such conflict, ``log_reopen`` should be scheduled on the same thread that performs ``log_flush``, and executed sequentially. One way of setting up signals to achieve this behavior requires masking the signal used for log rotation and having the thread check for pending signals using ``sigpending``.

Examples
--------

The debug module uses log to implement debug logging.

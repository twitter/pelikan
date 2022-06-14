ring array
==========

Ring array is a circular array data structure that allows elements to be
pushed/popped in FIFO order. This data structure is designed to facilitate the
sharing of resources between two threads with a producer/consumer relationship;
that is, one thread only pushes to the ring array and the other thread only pops
from it.

The main difference between ring array and ring buffer is that the former
organizes and processes data as elements, while the latter treats data as
flexible-length binary string.

Synopsis
--------
.. code-block:: C

  #include <cc_ring_array.h>

  struct ring_array *
  ring_array_create(size_t elem_size, uint32_t cap);

  void
  ring_array_destroy(struct ring_array **arr);

  rstatus_i
  ring_array_push(const void *elem, struct ring_array *arr);

  rstatus_i
  ring_array_pop(void *elem, struct ring_array *arr);

Description
-----------

This section contains descriptions of what the functions in the ccommon ring
array module do.

Creation/Destruction
^^^^^^^^^^^^^^^^^^^^
.. code-block:: C

   struct ring_array *ring_array_create(size_t elem_size, uint32_t cap);
   void ring_array_destroy(struct ring_array **arr);

In order to create a ccommon ``ring_array`` data structure, call
``ring_array_create()`` with ``elem_size`` as the ``sizeof`` the elements the
``ring_array`` contains and with ``cap`` as the maximum number of elements the
``ring_array`` should be able to hold. This function returns a pointer to the
``ring_array`` that it creates.

After the ``ring_array`` is no longer needed, ``ring_array_destroy`` should be
called with the ``ring_array`` as its argument to free the memory allocated for
it.

Element Access
^^^^^^^^^^^^^^
.. code-block:: C

   rstatus_i ring_array_push(const void *elem, struct ring_array *arr);
   rstatus_i ring_array_pop(void *elem, struct ring_array *arr);

These functions are used to push/pop elements in the ``ring_array``. To push an
element into the ``ring_array``, call ``ring_array_push()`` with ``elem`` being
a pointer to the element being stored and ``arr`` being the ``ring_array`` being
pushed to. ``ring_array_push()`` returns ``CC_OK`` if the element is stored, and
``CC_ERROR`` if the element could not be stored i.e. the ``ring_array`` is full.

To pop an element from the ``ring_array``, call ``ring_array_pop()`` with
``elem`` being a pointer to the memory location for where the element should be
popped to, and ``arr`` being the ``ring_array`` being popped from.
``ring_array_pop()`` returns ``CC_OK`` if the element was successfully popped,
and ``CC_ERROR`` if not successful (i.e. the ``ring_array`` is empty).

State
^^^^^
..code-block:: C
   bool ring_array_full(const struct ring_array *arr);
   bool ring_array_empty(const struct ring_array *arr);

These functions tell the caller about the state of the ``ring_array``,
specifically whether it is full or empty. In a producer/consumer model,
``ring_array_full()`` is a producer facing API, and ``ring_array_empty()`` is a
consumer facing API. This is so that the producer can check whether or not the
``ring_array`` is full before pushing more elements into the array; likewise, the
consumer can check whether or not the ``ring_array`` is empty before attempting
to pop.

Flush
^^^^^
..code-block:: C
   void ring_array_flush(struct ring_array *arr);

This function is an external API that discards everything in the ``ring_array``.


Examples
--------

Multi threaded ``Hello World!`` with ccommon ``ring_array``:

.. code-block:: c

   #include <cc_bstring.h>
   #include <cc_define.h>
   #include <cc_ring_array.h>

   #include <stdio.h>
   #include <stdlib.h>
   #include <string.h>
   #include <pthread.h>

   #define MESSAGE "Hello world!\n"

   struct msg_arg {
       struct ring_array *arr;
       struct bstring *msg;
   };

   static void *
   push_message(void *arg)
   {
       /* producer thread */
       struct ring_array *arr = ((struct msg_arg *)arg)->arr;
       struct bstring *msg = ((struct msg_arg *)arg)->msg;

       for (i = 0; i < msg->len;) {
           /* if there is space in the ring array, push next char in msg */
           if (!ring_array_full(arr)) {
               ring_array_push(&(msg->data[i++]), arr);
           }
       }

       return NULL;
   }

   int
   main(int argc, char **argv)
   {
       struct ring_array *arr;
       pthread_t producer = NULL;
       struct bstring msg = { sizeof(MESSAGE), MESSAGE };
       struct msg_arg args;

       arr = ring_array_create(sizeof(char), 5);

       /* share array with producer thread */
       args.arr = arr;
       args.msg = &msg;

       /* create producer thread */
       pthread_create(&producer, NULL, &push_message, &args);

       /* consume from arr */
       for (i = 0; i < msg.len;) {
           if (!ring_array_empty(arr)) {
                char c;
                ring_array_pop(&c, arr);
                printf("%c", c);
                ++i
           }
       }

       /* Destroy ring_array */
       ring_array_destroy(&arr);

       return 0;
   }

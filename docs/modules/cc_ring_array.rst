ccommon ring array
==================

The ccommon ring array is a circular array data structure that allows elements to be pushed/popped in FIFO order. This data structure is designed to facilitate the sharing of resources between two threads with a producer/consumer relationship; that is, one thread only pushes to te ring array and the other thread only pops from the ring array.

Synopsis
--------
::

  #include <cc_ring_array.h>

  struct ring_array *
  ring_array_create(size_t elem_size, uint32_t cap);

  void
  ring_array_destroy(struct ring_array *arr);

  rstatus_t
  ring_array_push(const void *elem, struct ring_array *arr);

  rstatus_t
  ring_array_pop(void *elem, struct ring_array *arr);

Description
-----------

This section contains descriptions of what the functions in the ccommon ring array module do.

Creation/Destruction
^^^^^^^^^^^^^^^^^^^^
::

   struct ring_array *ring_array_create(size_t elem_size, uint32_t cap);
   void ring_array_destroy(struct ring_array *arr);

In order to create a ccommon ``ring_array`` data structure, call ``ring_array_create()`` with ``elem_size`` as the ``sizeof`` the elements the ``ring_array`` contains and with ``cap`` as the maximum number of elements the ``ring_array`` should be able to hold. This function returns a pointer to the ``ring_array`` that it creates.

After the ``ring_array`` is no longer needed, ``ring_array_destroy`` should be called with the ``ring_array`` as its argument to free the memory allocated for it.

Element Access
^^^^^^^^^^^^^^
::

   rstatus_t ring_array_push(const void *elem, struct ring_array *arr);
   rstatus_t ring_array_pop(void *elem, struct ring_array *arr);

These functions are used to push/pop elements in the ``ring_array``. To push an element into the ``ring_array``, call ``ring_array_push()`` with ``elem`` being a pointer to the element being stored and ``arr`` being the ``ring_array`` being pushed to. ``ring_array_push()`` returns ``CC_OK`` if the element is stored, and ``CC_ERROR`` if the element could not be stored (i.e. the ``ring_array`` is full).

To pop an element from the ``ring_array``, call ``ring_array_pop()`` with ``elem`` being a pointer to the memory location for where the element should be popped to, and ``arr`` being the ``ring_array`` being popped from. ``ring_array_pop()`` returns ``CC_OK`` if the element was successfully popped, and ``CC_ERROR`` if not successful (i.e. the ``ring_array`` is empty).

Examples
--------

Hello World! with ccommon ``ring_array``:

.. code-block:: c

                #include <cc_define.h>
                #include <cc_ring_array.h>

                #include <stdio.h>
                #include <stdlib.h>
                #include <string.h>

                int
                main(int argc, char **argv)
                {
                    struct ring_array *arr;
                    char c, *msg = "Hello world!\n";
                    int i, msg_len = strlen(msg);
                    rstatus_t status;

                    /* Create ring_array */
                    arr = ring_array_create(sizeof(char), 100);

                    /* Push message into ring_array */
                    for (i = 0; i < msg_len; ++i) {
                        status = ring_array_push(msg + i, arr);

                        if (status != CC_OK) {
                            printf("Could not push message!\n");
                            exit(1);
                        }
                    }

                    /* Pop chars stored in arr and print them */
                    for (i = 0; i < msg_len; ++i) {
                        status = ring_array_pop(&c, arr);

                        if (status != CC_OK) {
                            printf("Could not pop entire message!");
                            exit(1)
                        }

                        printf("%c", c);
                    }

                    /* Destroy ring_array */
                    ring_array_destroy(arr);

                    return 0;
                }

Introduction
============

The C Style Guide is a set of guidelines and conventions that encourage
good code. While some suggestions are more strict than others, you should
always practice good judgement.

If following the guide causes unnecessary hoop-jumping or otherwise
less-readable code, *readability trumps the guide*. However, if the more
readable variant comes with perils or pitfalls, readability may be
sacrificed.

Consistency is crucial. Without consistent application, there simply is no style
to speak of [#fn1]_. Stay in sync with the rest of the codebase; when you want
to change a rule or style, change it everywhere.

Contents
--------

.. toctree::
   :maxdepth: 1

   coding_style


C Standard
==========

- Use ``-std=c11`` when compiling
- Avoid ``_Atomic``, ``_Generic`` and ``_Thread_local``, for now, we will
  embrace ``C11`` fully when Twitter's official ``GCC`` is bumped to 4.9.

Indentation
===========

- Do not use literal tabs. Expand tabs to **four** spaces instead.
- Use **four** spaces for every indentation level.
- Do not use more than **three** levels of indentation.
- Make sure that your editor does not leave space at the end of each line.


Naming
======

- Use ``snake_case`` for the names of variables, functions, and files.
- Use your own judgement when you name variables and be as spartan as possible,
  abbreviation is common in C.
  For example, do not use a name like ``this_variable_is_a_temporary_counter``.


Types
=====

- Do not use the following types:

  - ``int``
  - ``char``
  - ``short``
  - ``long``

  Instead, include the ``<stdint.h>`` header and use the following types:

  - ``int8_t``
  - ``uint8_t``
  - ``int16_t``
  - ``uint16_t``
  - ``int32_t``
  - ``uint32_t``
  - ``int64_t``
  - ``uint64_t``

- Use the `bool` type for boolean data. You have to include the ``<stdbool.h>``
  header.
- Always use the ``size_t`` type when you work with:

  - Sizes of objects
  - Memory ranges


Line length
===========

- Limit each line to 80 columns or less.
- If you have to wrap a longer statement, put the operator at the end of the
  line and use **four** spaces to indent the next line. For example:

  .. code-block:: c

        while (cnt < 20 && this_variable_name_is_too_long &&
            ep != NULL) {
                z = a + really + long + statement + that + needs +
                    two + lines + gets + indented + four + spaces +
                    on + the + second + and + subsequent + lines;
        }

  and:

  .. code-block:: c

    int a = function(param_a, param_b, param_c, param_d, param_e,
                         param_f, param_g, param_h, param_i,
                         param_j, param_k, param_l);


Braces
======

- Always use braces for all conditional blocks (``if``, ``switch``, ``for``,
  ``while``, and ``do``), even for single statement conditional blocks (remember
  the ``goto fail`` bug by Apple?). For example:

  .. code-block:: c

        if (cond) {
            stmt;
        }

- For non-function statement blocks, put the opening brace at the end of the
  first line and the closing brace in a new line. For example:

  .. code-block:: c

        if (x is true) {
            we do y
        }

- For functions, put the opening brace at the beginning of the second line
  and the closing brace in a new line. For example:

  .. code-block:: c

        int
        function(int x)
        {
            body of the function
        }

- Place the closing brace in its own line, except when it is part of the same
  statement, such as a ``while`` in a ``do``-statement or an ``else`` in
  an ``if``-statement. For example:

  .. code-block:: c

        do {
            body of do-loop
        } while (condition);

        and,

        if (x == y) {
            ..
        } else if (x > y) {
            ...
        } else {
            ....
        }


Switch alignment
================

Align the ``switch`` keyword and the corresponding ``case`` and ``default``
keywords to the same column. For example:

.. code-block:: c

      switch (alphabet) {
      case 'a':
      case 'b':
          printf("I am a or b\n");
          break;
      default:
          break;
      }


Infinite loops
=============

Create infinite loops with ``for`` statements, not ``while`` statements.
For example:

.. code-block:: c

      for (;;) {
          stmt;
      }


Spaces
======

- Do not use a space after a function name.
- Use space after keywords, except after the ``sizeof``, ``typeof``, ``alignof``
  and ``__attribute__`` keywords, since they are used like functions.
- Do not add spaces inside parenthesized expressions. For example:

  .. code-block:: c

        s = sizeof( sizeof(*p)) ); /* Bad example */
        s = sizeof(sizeof(*p)); /* Good example */

- When declaring pointers, place the asterisk ('*'') adjacent to the variable
  name, not the type name. For example:

  .. code-block:: c

        int
        function(int *p)
        {
            char *p;
            body of the function
        }

- Use one space around most binary and ternary operators, such as any of these:

  ``=``  ``+``  ``-``  ``<``  ``>``  ``*``  ``/``  ``%``  ``|``  ``&`` ``^``
  ``<=``  ``>=``  ``==``  ``!=``  ``?``  ``:``

  Do not add spaces after unary operators:

  ``&``  ``*``  ``+``  ``-``  ``~``  ``!``  ``sizeof``  ``typeof``  ``alignof``
  ``__attribute__``  ``defined``

  Do not add spaces before the postfix increment and decrement unary operators:

  ``++``  ``--``

  Do not add spaces around the ``.`` and ``->`` structure member operators.

- Do not add spaces after casts. For example:

  .. code-block:: c

        int q = *(int *)&p


Type definitions
================

Do not use ``typedef`` for structure types. Typedefs are problematic
because they do not properly hide their underlying type; for example, you
need to know if the typedef is the structure itself or a pointer to the
structure. In addition, they must be declared exactly once, whereas an
incomplete structure type can be mentioned as many times as necessary.
Typedefs are difficult to use in stand-alone header files: the header
that defines the typedef must be included before the header that uses it,
or by the header that uses it (which causes namespace pollution), or
there must be a back-door mechanism for obtaining the typedef.

The only exception for using a ``typedef`` is when defining a type for a
function pointer.


Functions
=========

- Declare functions that are local to a file as static.
- Place function types in their own line preceding the function. For example:

  .. code-block:: c

        static char *
        function(int a1, int a2, float fl, int a4)
        {
            ...

- Separate two successive functions with one blank line.
- Include parameter names with their datypes in the function declaration. For
  example:

  .. code-block:: c

        void function(int param);

- When you use a wrapper function, name the wrapped function with the same name
  as the wrapper function preceded by an underscore ('_'). Wrapped functions
  are usually static. For example:

  .. code-block:: c

        static int
        _fib(int n)
        {
            ...
        }
        int
        fib(int n)
        {
            ...
            _fib(n);
            ...
        }

- Create functions that are short and sweet. Functions should do just one
  thing and fit on one or two screenfuls of text (80x24 screen size).

  The maximum length of a function is inversely proportional to the
  complexity and indentation level of that function. So, if you have a
  conceptually simple function that is just one long (but simple)
  case-statement, where you have to do lots of small things for many
  different cases, it is acceptable to have a longer function.

  Another measure of function complexity is the number of local variables. They
  should not exceed 5-10. If your function has more than that, re-think the
  function and split it into smaller pieces. A human brain can
  generally easily keep track of about seven different things; anything more
  and it gets confused. You may need to come back to your function and
  understand what you did two weeks from now.


Goto statements
===============

- Use ``goto`` statements judiciously. Never use them to jump out of the
  current function. Almost the only case where ``goto`` statements are helpful
  is when a flow can exit from multiple locations within a function, and the
  same clean-up logic applies to all of them.


  .. code-block:: c

        int
        fun(void)
        {
            int result = 0;
            char *buffer;
            buffer = malloc(1024);
            if (buffer == NULL) {
                return -1;
            }
            if (condition1) {
                while (loop1) {
                    ...
                }
                result = 1;
                goto out;
            }
            ...
        out:
            free(buffer);
            return result;
        }



Comments
========

- Do not use ``//`` for single line comments. Instead, use the ``/* ... */``
  style.

- For multi-line comments, use the following style:

  .. code-block:: c

        /*
         * This is the preferred style for multi-line
         * comments in the Linux kernel source code.
         * Please use it consistently.
         *
         * Description:  A column of asterisks on the left side,
         * with beginning and ending almost-blank lines.
         */

- To comment out blocks of code spanning several lines, use
  ``#ifdef 0 ... #endif``.

- Add comments before all major functions to describe what they do. Do not put
  comments in the function body unless absolutely needed. For example:

  .. code-block:: c

        /*
         * Try to acquire a physical address lock while a pmap is locked.  If we
         * fail to trylock we unlock and lock the pmap directly and cache the
         * locked pa in *locked.  The caller should then restart their loop in
         * case the virtual to physical mapping has changed.
         */
        int
        vm_page_pa_tryrelock(pmap_t pmap, vm_paddr_t pa, vm_paddr_t *locked)
        {
            ...

- Use only one data declaration per line. (Do not use commas for multiple data
  declarations.) This leaves you room for a small comment on each
  item that explains its use.


Other naming conventions
========================

- Use UPPERCASE for macro names.

- Use ``enum`` to define several related constants. Use UPPERCASE for all
  enumeration values.

- Avoid macros as much as possible and use inline functions wherever you can.

- For macros encapsulating compound statements, right-justify the backslashes
  and enclose the statements in a ``do { ... } while (0)`` block.

- For parameterized macros, add parentheses to all the parameters. For example:

  .. code-block:: c

        #define ADD_1(x) ((x) + 1)



Inclusion
=========

- Rule of thumb- local to global: first include header of the same name as
  source, followed by headers in the same project, and external/system headers
  last.
- Organize header inclusion in blocks, separated by blank line(s). For example,
  headers that are shipped with the project and system headers should be in
  separate clusters.
- Sort inclusions within the same block in alphabetic order.
  .. code-block:: c

        /* File: foo.c */
        #include "foo.h" /* first block: own header */

        #include "bar.h" /* second block: headers from current project */
        #include "util/baz.h"

        #include <stdbool.h> /* third block: system/library headers */
        #include <stdint.h>
        #include <stdlib.h>


Structures
==========

- To determine the size of a data structure, use some data of that type instead
  of the type itself. For example:

  .. code-block:: c

        char *p;
        p = malloc(sizeof(*p))  /* Good example */
        p = malloc(sizeof(char) /* Bad example */

- Declare each variable in a structure in a separate line. Try to make the
  structure readable by aligning the member names using either tabs or spaces.
  Use only one space or tab if it suffices to align at least ninety percent
  of the member names. Separate names that follow extremely long types with
  a single space.

  .. code-block:: c

        struct foo {
            struct foo    *next;   /* List of active foo. */
            struct mumble amumble; /* Comment for mumble. */
            int           bar;     /* Try to align the comments. */
            struct verylongtypename *baz; /* Won't fit in 2 tabs. */
        };
        struct foo *foohead;    /* Head of global foo list. */

- Declare major structures at the top of the file in which they are used, or
  in separate header files if they are used in multiple source files.
  Use of the structures should be by separate declarations and should be
  ``extern`` if they are declared in a header file.


Pointers
========

- Use ``NULL`` as the null pointer constant (instead of ``0``).

- Compare pointers to ``NULL``. For example:

  .. code-block:: c

        (p = f()) == NULL

  Do not compare to zero. For example:

  .. code-block:: c

        !(p = f())

- Do not use ``!`` for comparisons (unless the variable is of boolean type). For
  example:

  .. code-block:: c

        if (*p == '\0')

  The following snippet is a bad example:

  .. code-block:: c

        if (!*p)

- Use ``const`` for function parameters if the pointer has no side effect.

- Functions in charge of freeing an object should take a pointer to the intended
  pointer to be freed, and set the pointer to the object to ``NULL`` before
  returning. This prevents dangling pointers that are often discovered long
  after ``free`` is called.

  .. code-block:: c

        void
        destroy_buffer(struct buffer **pb)
        {
            free(*pb);
            *pb = NULL;
        }

- Dynamically allocated structures should always initialize their members of
  pointer type as soon as possible, to avoid the dangling pointer problem.


Macros
======

- Prefer ``static inline`` functions over macros. Macros often have unintended
  side effects, for example:

  .. code-block:: c

        #define MAX(a,b) ((a) > (b) ? (a) : (b))

  When used as in ``MAX(x++, y++)``, will increment either ``x`` or ``y`` twice,
  which is probably not intended by the caller.


.. rubric:: Footnotes

.. [#fn1] Frederick Brooks gave a definition of "style" in his book, The Design
   of Design, which begins with "Style is a set of different repeated
   microdecisions...". The book talked about the importance of Consistency in
   the pages leading to this definition, starting from page 142, where the
   author claimed that "consistency underlies all principles of quality".


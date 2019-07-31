Option
======

The option module facilitates the declaration, loading and combination of per-module and global configuration options.

The ability to compose options minimizes the work developers put into related boilerplate, and keeps naming and default values consistent.

Background
----------

Every service needs some amount of configuration. While hugely useful and necessary, code that does configuration is mostly boilerplate and isn't exactly fun to write. Furthermore, reusing libraries or modules often does not spare the developers from creating and passing along aggregated options for these components, repeated for each service or application. Such boilerplate thus often takes a large code footprint among application-specific logic despite being mundane.

When it comes to the format of configuration, there are flavors and varieties. Most notable is the choice of using primarily command line options or config files. In general, command line options are easier and quicker to start with, making them great for testing and development. However, they become less manageable as the number of options increases. And as part of the command that launched the process, they are potentially confusing if applications ever want config-reload support. Config files work better if a large of tuning knobs are present and commonly used, or when it is desirable to keep history or share configurations. On the downside, an extra file is used.

Within each route there are also varieties. Several conventions exist for command line, such as short vs long name, single or double dash prefix. For config files, many languages and/or formats have been used, such as YAML, XML, and numerous simpler formats created ad hoc by the developers.

Goals
-----

We want to have configuration options that:

#. minimize boilerplate code by applications that use our library or modules.
#. can easily be archived, shared and templated.

Design
------

Options are part of each module, declared with macro with a table-like layout. Information of each option include name, type, default value, and description. They can be combined and manipulated with other macros and functions.

Options are grouped and viewed in two ways: as members of a higher-level struct, or elements in an array. The former makes it easy to access individual option by name, such as passing the right options to initialize a specific module. The latter facilitates option traversal, which is useful to print all the options available and their default/current values.

We opt for a file-oriented approach to pass most options into an application. Our estimate of the number of options in the services we want to create makes config files the more readable choice, and it paves way for dynamic reload.

The format within the config file needs to balance a few things: expressiveness, human readability, and parsing complexity. We researched a few existing solutions: JSON, YAML, Java's simple Option class, whitespace delimiter-ed options used by Redis, etc. Looking at the needs of our target application group, as represented by Memcached and Redis, it seems the simple key value is either sufficient or "almost sufficient". Thus we choose to use a key/value format, ``<KEY>: <VALUE>`` (colon and space ``: ``), which happens to be the YAML mappings format. We also adopted the YAML comment that begins with ``#``. It is quite trivial to write a parser for these basic structures from scratch. And being YAML-compatible makes two other things possible: 1) loading the config file by any YAML parser, and 2) extending the config format by borrowing more from the YAML protocol or even using a full-fledged YAML library, if ever needed.

Data Structure
--------------
.. code-block:: C

  typedef enum option_type {
      OPTION_TYPE_BOOL,
      OPTION_TYPE_UINT,
      OPTION_TYPE_FPN,
      OPTION_TYPE_STR,
      OPTION_TYPE_SENTINEL
  } option_type_e;

  typedef union option_val {
      bool vbool;
      uintmax_t vuint;
      double vfpn;
      char *vstr;
  } option_val_u;

  struct option {
      char *name;
      bool set;
      option_type_e type;
      option_val_u default_val;
      option_val_u val;
      char *description;
  };

The core data structure ``struct option`` has six members. ``name`` and ``description`` help identify and explain the purpose of the option. ``type`` decides how input should be interpreted, which currently can be boolean, unsigned integer, double or C string. Both the default value and current value are kept around, with values matching the type. Keeping the default separately will make it easy to reset the option to original. Finally, boolean ``set`` tells if an option has been set, and thus usable.

Synopsis
--------
.. code-block:: C

  rstatus_i option_set(struct option *opt, char *val_str);
  rstatus_i option_default(struct option *opt);
  rstatus_i option_load_default(struct option options[], unsigned int nopt);
  rstatus_i option_load_file(FILE *fp, struct option options[], unsigned int nopt);

  void option_print(struct option *opt);
  void option_print_all(struct option options[], unsigned int nopt);
  void option_describe_all(struct option options[], unsigned int nopt);

  void option_free(struct option options[], unsigned int nopt);

Usage
-----

Declare and initialize
^^^^^^^^^^^^^^^^^^^^^^
.. code-block:: C

  #define OPTION_DECLARE(_name, _type, _default, _description)
  #define OPTION_INIT(_name, _type, _default, _description)

To use these macros, ``_name`` *must* be a legal identifier [C11]_. See ``cc_option.h`` for related implementation details.

A C preprocessor convention allows the above macros to be applied against a "list" of options. For example, one can define options, ``BUF_OPTION``, for a buffer module as such:

.. code-block:: C

  #define BUF_OPTION(ACTION)                                                                              \
      ACTION( buf_init_size,  OPTION_TYPE_UINT,   BUF_DEFAULT_SIZE,   "default size when buf is created" )\
      ACTION( buf_poolsize,   OPTION_TYPE_UINT,   BUF_POOLSIZE,       "buf pool size"                    )

An option struct for the buffer module can be defined by using the ``OPTION_DECLARE`` macro against the list above:

.. code-block:: C

  typedef struct {
      BUF_OPTION(METRIC_DECLARE)
  } buf_options_st;

Set option values
^^^^^^^^^^^^^^^^^
.. code-block:: C

  rstatus_i option_set(struct option *opt, char *val_str);
  rstatus_i option_default(struct option *opt);
  rstatus_i option_load_default(struct option options[], unsigned int nopt);
  rstatus_i option_load_file(FILE *fp, struct option options[], unsigned int nopt);

Print option info
^^^^^^^^^^^^^^^^^
.. code-block:: C

  void option_print(struct option *opt);
  void option_print_all(struct option options[], unsigned int nopt);
  void option_describe_all(struct option options[], unsigned int nopt);


Examples
--------


References
----------
.. [C11] `C11 standard <http://www.open-std.org/jtc1/sc22/wg14/www/standards.html#9899>`_

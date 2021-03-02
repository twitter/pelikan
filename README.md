#### Pelikan is Twitter's unified cache server.

# Content
* [Overview](#overview)
  * [Products](#products)
  * [Features](#features)
* [Build](#building-pelikan)
* [Usage](#usage)
* [Community](#community)
  * [Stay in touch](#stay-in-touch)
  * [Contributing](#contributing)
* [Documentation](#documentation)
* [License](#license)

[![Build Status](https://travis-ci.org/twitter/pelikan.svg?branch=master)](https://travis-ci.org/twitter/pelikan)

# Overview
After years of using and working on various cache services, we built a common
framework that reveals the inherent architectural similarity among them.

By creating well-defined modules, most of the low-level functionalities are
reused as we create different binaries. The implementation learns from our
operational experiences to improve performance and reliability, and leads to
software designed for large-scale deployment.

The framework approach allows us to develop new features and protocols quickly.

## Products
Currently Pelikan yields three main products, all of which are backends/servers.
- `pelikan_twemcache`: a Twemcache replacement
- `pelikan_slimcache`: a Memcached-like server with ultra-low memory overhead-
  compared to Memcached/Redis, the per-key overhead is reduced by up to 90%
- `pelikan_pingserver`: an over-engineered, production-ready ping server useful
  as a tutorial and for measuring baseline RPC performance
- [Experimental]`pelikan_segcache`: a Memcached-like server with extremely high
  memory efficiency and excellent core scalability. See our [NSDI'21 paper](https://www.usenix.org/conference/nsdi21/presentation/yang-juncheng)
  for design and evaluation details.

## Features
- runtime separation of control and data plane
- predictably low latencies via lockless data structures, worker never blocks
- per-module config options and metrics that can be composed easily
- multiple storage and protocol implementations, easy to further extend
- low-overhead command logger for hotkey and other important data analysis

# Building Pelikan

## Requirement
- platform: Mac OS X or Linux
- build tools: `cmake (>=2.8)`
- compiler: `gcc (>=4.8)` or `clang (>=3.1)`
- (optional) unit testing framework: `check (>=0.10.0)`. See below.

## Build
```sh
git clone https://github.com/twitter/pelikan.git
mkdir _build && cd _build
cmake ..
make -j
```
The executables can be found under ``_bin/`` (under build directory)

To run all the tests, including those on `ccommon`, run:
```sh
make test
```

To skip building tests, replace the `cmake` step with the following:
```sh
cmake -DCHECK_WORKING=off ..
```
## Install `check`
To compile and run tests, you will have to install [check](http://libcheck.github.io/check/).
Please follow instructions in the project.

**Note**: we highly recommend installing the latest version of `check` from
source, as there are, unfortunately, a [linker bug](https://sourceforge.net/p/check/mailman/message/32835594/)
in packages installed by the current versions of `brew` (OS X),
`CentOS` and `Ubuntu LTS`. The bug does not affect building executables.


# Usage
Using `pelikan_twemcache` as an example, other executables are highly similar.

To get info of the service, including usage format and options, run:
```sh
_bin/pelikan_twemcache -h
```

To launch the service with default settings, simply run:
```sh
_bin/pelikan_twemcache
```

To launch the service with the sample config file, run:
```sh
_bin/pelikan_twemcache config/twemcache.conf
```

You should be able to try out the server using an existing memcached client,
or simply with `telnet`.
```sh
$ telnet localhost 12321
Trying 127.0.0.1...
Connected to localhost.
Escape character is '^]'.
set foo 0 0 3
bar
STORED
```

**Attention**: use `admin` port for all non-data commands.
```sh
$ telnet localhost 9999
Trying 127.0.0.1...
Connected to localhost.
Escape character is '^]'.
version
VERSION 0.1.0
stats
STAT pid 54937
STAT time 1459634909
STAT uptime 22
STAT version 100
STAT ru_stime 0.019172
...
```

## Configuration

Pelikan is file-first when it comes to configurations, and currently is
config-file only. You can create a new config file following the examples
included under the `config` directory.

**Tip**: to get a list of config options for each executable, use `-c` option:
```sh
_bin/pelikan_twemcache -c
```


# Community

## Stay in touch
- Join our project chat on [![Zulip](https://img.shields.io/badge/zulip-join_chat-brightgreen.svg)](https://pelikan.zulipchat.com/)
  for questions and discussions
- Follow us on Twitter: [@pelikan_cache](https://twitter.com/pelikan_cache)
- Visit <http://pelikan.io>

## Contributing

Please take a look at our [community manifesto](https://github.com/twitter/pelikan/blob/master/docs/manifesto.rst)
and [coding style guide](https://github.com/twitter/pelikan/blob/master/docs/coding_style.rst).

If you want to submit a patch, please follow these steps:

1. create a new issue
2. fork on github & clone your fork
3. create a feature branch on your fork
4. push your feature branch
5. create a pull request linked to the issue


# Documentation
We have made progress and are actively working on documentation, and will put it
on our website. Meanwhile, check out the current material under `docs/`

## License
This software is licensed under the Apache 2.0 license, see [LICENSE](LICENSE) for details.

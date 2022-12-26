# Pelikan


**Note: After Nov 17, 2022, maintainers of the Pelikan project have lost access
to the Twitter Github. Active development of Pelikan will continue at
[pelikan-io/pelikan](https://github.com/pelikan-io/pelikan). Please update your bookmark, thanks!**

Pelikan is Twitter's framework for developing cache services. It is:

* **Fast**: Pelikan provides high-throughput and low-latency caching solutions.

* **Reliable**: Pelikan is designed for large-scale deployment and the
  implementation is informed by our operational experiences.

* **Modular**: Pelikan is a framework for rapidly developing new caching
  solutions by focusing on the inherent architectural similarity between caching
  services and providing reusable low-level components.

[![License: Apache-2.0][license-badge]][license-url]
[![Build Status][cargo-build-badge]][cargo-build-url]
[![Fuzz Status][cargo-fuzz-badge]][cargo-fuzz-url]
[![Zulip Chat][zulip-badge]][zulip-url]

[Website](http://pelikan.io) |
[Chat][zulip-url]

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

# Overview
After years of using and working on various cache services, we built a common
framework that reveals the inherent architectural similarity among them.

By creating well-defined modules, most of the low-level functionalities are
reused as we create different binaries. The implementation learns from our
operational experiences to improve performance and reliability, and leads to
software designed for large-scale deployment.

The framework approach allows us to develop new features and protocols quickly.

## Products
Pelikan contains the following products:
- `pelikan_segcache_rs`: a Memcached-like server with extremely high memory
  efficiency and excellent core scalability. See our [NSDI'21 paper] for design
  and evaluation details.
- `pelikan_pingserver_rs`: an over-engineered, production-ready ping server
  useful as a tutorial and for measuring baseline RPC performance
- [`momento_proxy`][momento_proxy-url]: a proxy which allows existing 
  applications to use Momento instead of a Memcache-compatible cache backend.

## Legacy
Pelikan legacy codebase can be found within the `legacy` folder of this project.
It is composed of the original C codebase and backend implementations. It
remains as a reference, but is not recommended for production deployments.

## Features
- runtime separation of control and data plane
- predictably low latencies via lockless data structures, worker never blocks
- per-module config options and metrics that can be composed easily
- multiple storage and protocol implementations, easy to further extend
- low-overhead command logger for hotkey and other important data analysis

# Building Pelikan

## Requirement
- Rust [stable toolchain](https://www.rust-lang.org/learn/get-started)
- C toolchain: `llvm/clang (>= 7.0)`
- Build tools: `cmake (>= 3.2)`

## Build
```sh
git clone https://github.com/twitter/pelikan.git
cd pelikan
cargo build --release
```

## Tests
```sh
cargo test
```

# Usage
Using `pelikan_segcache_rs` as an example, other executables are highly similar.

To get info of the service, including usage format and options, run:
```sh
target/release/pelikan_segcache_rs --help
```

To launch the service with default settings, simply run:
```sh
target/release/pelikan_segcache_rs
```

To launch the service with the sample config file, run:
```sh
target/release/pelikan_segcache_rs config/segcache.toml
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


# Community

## Stay in touch
- Join our project chat on [![Zulip][zulip-badge]][zulip-url]
  for questions and discussions
- Follow us on Twitter: [@pelikan_cache]
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

[@pelikan_cache]: https://twitter.com/pelikan_cache
[cargo-build-badge]: https://img.shields.io/github/workflow/status/twitter/pelikan/cargo-build/master?label=build
[cargo-build-url]: https://github.com/twitter/pelikan/actions/workflows/cargo.yml?query=branch%3Amaster+event%3Apush
[cargo-fuzz-badge]: https://img.shields.io/github/workflow/status/twitter/pelikan/cargo-fuzz/master?label=fuzz
[cargo-fuzz-url]: https://github.com/twitter/pelikan/actions/workflows/fuzz.yml?query=branch%3Amaster+event%3Apush
[check]: (http://libcheck.github.io/check/)
[check-linker-bug]: (https://sourceforge.net/p/check/mailman/message/32835594/)
[license-badge]: https://img.shields.io/badge/license-Apache%202.0-blue.svg
[license-url]: https://github.com/twitter/pelikan/blob/master/LICENSE
[momento_proxy-url]: src/proxy/momento/README.md
[NSDI'21 paper]: https://www.usenix.org/conference/nsdi21/presentation/yang-juncheng
[zulip-badge]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[zulip-url]: https://pelikan.zulipchat.com/

# Momento Proxy

This product is a simple proxy which can allow existing applications which use
Memcached set/get operations for caching to use [Momento](https://momentohq.com)
cache offering without any code changes.

## Features

* **Transparent**: allows existing applications which use Memcached to switch to
  Momento without code changes.
* **Stats**: get insight into runtime by using the Memcached `stats` command on
  the admin port.
* **Command Log**: enables logging of commands for audit and offline workload
  analysis.

## Limitations

* Only `get` and `set` operations are supported.

## Building

Follow the [build steps](../../README.md#building-pelikan-rust) in the readme.

## Configuration

### Authentication Token

The Momento proxy requires that the `MOMENTO_AUTHENTICATION` environment
variable is set and contains a valid Momento authentication token.

If you're new to Momento, you should refer to the
[Momento CLI docs](https://github.com/momentohq/momento-cli#momento-cli) for
instructions to sign up and get an authentication token.

### Create Cache

After obtaining an authentication token, you should create one or more caches to
use with the proxy.

### Proxy Configuration

The Momento proxy uses a TOML configuration file. As there aren't any sensible
defaults, we require that you provide a configuration file when using the proxy.
See the [example config](../../config/momento-proxy.toml) and modify it to suit
your requirements.

## Running

After completing the build and configuration, you are ready to run the Momento
proxy.

```cargo run --release --bin momento_proxy -- path/to/config.toml```

Your application can now connect to the proxy on the configured port(s) to send
requests to the corresponding Momento cache(s).

The resulting binary will be `target/release/momento_proxy` and it can be copied
to a standard system path (eg: `/usr/local/bin`).

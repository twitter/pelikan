The idea of a CLI client comes from Redis, which builds `resp-cli` for easy
testing and interactive play. This is particularly useful for Redis because
the RESP protocol is more verbose than Memcached's ASCII protocol.

The command line prompt and CLI command format are both modeled after Redis.
We may also borrow code from `redis-cli.c` in the future. We want to
acknowledge the fact that redis-cli is an ongoing inspiration.

Since we only shallowly support the syntax portion of the Redis protocol, which
is RESP, the binary and related files are named accordingly to reflect that.
Actual command supported by the server may differ from one binary to another,
and may change over time as well.

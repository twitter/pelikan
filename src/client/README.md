The idea of a CLI client comes from Redis, which builds `redis-cli` for easy
testing and interactive play. This is particularly useful for Redis because
the RESP protocol is more verbose than Memcached's ASCII protocol.

The command line prompt and CLI command format are both modeled after Redis.
We also borrow code from `redis-cli.c`, mostly around these two areas. Due to
style difference, the code has to be reformatted or rewritten to follow the
style guide, so we would like to explicitly acknowledge code reuse in this file.

## Examples

For PMEM usage use `-m` parameter followed by a list of PMEM mount point paths. Without that parameter configs will be created for RAM usage.
Note that the first mount point provided will be bound to the numa node 0, next mount point to the numa node 1, etc.
Put a list of mount points in quotes when providing more than one path.

Twemcache:

```
# generating configs without PMEM support
./generate.sh -s -p pelikan/_build/_bin/pelikan_twemcache -c -r rpc-perf/target/release/rpc-perf -t 127.0.0.1

# generating configs with PMEM support for two mount points
./generate.sh -s -p pelikan/_build/_bin/pelikan_twemcache -c -r rpc-perf/target/release/rpc-perf -t 127.0.0.1 -m "/mnt/pmem0 /mnt/pmem1"
```

Slimcache:

```
# generating configs without PMEM support
./generate.sh -s -p pelikan/_build/_bin/pelikan_slimcache -c -r rpc-perf/target/release/rpc-perf -t 127.0.0.1

# generating configs with PMEM support for two mount points
./generate.sh -s -p pelikan/_build/_bin/pelikan_slimcache -c -r rpc-perf/target/release/rpc-perf -t 127.0.0.1 -m "/mnt/pmem0 /mnt/pmem1"
```

To run benchmarks provide paths to generated config directories:
```
./runtest.sh -c rpcperf_100_1024_4 -s pelikan_1024_4 -t 127.0.0.1
```

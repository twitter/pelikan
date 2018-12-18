#!/bin/bash

conn_configs=(100 1000 10000)
mem_configs=(4 8 16 32)
size_configs=(64 128 256 512 1024 2048)
instances=30
ksize=32
rate=100000
threads=2

# pelikan configs
for size in "${size_configs[@]}"
do
    vsize=$((size - ksize))
    for mem in "${mem_configs[@]}"
    do
        slab_mem=$((mem * 1024 * 1024 * 1024))
        prefix=pelikan_${size}_${mem}
        python server_config.py --prefix="$prefix" --instances="$instances" --slab_mem "$slab_mem" --vsize "$vsize"
    done
done

# rpc-perf configs
for conn in "${conn_configs[@]}"
do
    for size in "${size_configs[@]}"
    do
        vsize=$((size - ksize))
        for mem in "${mem_configs[@]}"
        do
            slab_mem=$((mem * 1024 * 1024 * 1024))
            prefix=rpcperf_${conn}_${size}_${mem}
            python client_config.py --prefix="$prefix" --instances="$instances" --rate="$rate" --connections="$conn" --vsize "$vsize" --slab_mem="$slab_mem" --threads="$threads"
        done
    done
done

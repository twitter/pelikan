#!/bin/bash

mem_configs=(4 8 16 32)
size_configs=(32 64 128 256 512 1024 2048)
instances=30

for size in "${size_configs[@]}"
do
    for mem in "${mem_configs[@]}"
    do
        slab_mem=$((mem * 1024 * 1024 * 1024))
        prefix=test_${size}_${mem}
        python server_config.py --prefix="$prefix" --instances="$instances" --slab_mem "$slab_mem" --size "$size"
    done
done

#!/bin/bash

conn_configs=(100 1000 10000)
mem_configs=(4 8 16 32)
size_configs=(64 128 256 512 1024 2048)
instances=30
ksize=32
rate=100000
threads=2

# Initialize our own variables:
client=false
server=false
binary="pelikan_twemcache"
target="127.0.0.1"

show_help()
{
    echo "generate.sh [-c [-t target/serverIP]] [-s [-b binary]]"
}

get_args()
{
    while getopts ":b:t:csh" opt; do
        case "$opt" in
        c)  client=true
            ;;
        s)  server=true
            ;;
        t)  binary=$OPTARG
            ;;
        t)  target=$OPTARG
            ;;
        h)
            show_help
            exit 0
            ;;
        \?)
            echo "unrecognized option $opt"
            show_help
            exit 1
            ;;
        esac
    done
}

# pelikan configs
gen_pelikan()
{
    for size in "${size_configs[@]}"
    do
        vsize=$((size - ksize))
        for mem in "${mem_configs[@]}"
        do
            slab_mem=$((mem * 1024 * 1024 * 1024))
            prefix=pelikan_${size}_${mem}
            python server_config.py --prefix="$prefix" --binary="$binary" --instances="$instances" --slab_mem "$slab_mem" --vsize "$vsize"
        done
    done
}

# rpc-perf configs
gen_rpcperf()
{
    for conn in "${conn_configs[@]}"
    do
        for size in "${size_configs[@]}"
        do
            vsize=$((size - ksize))
            for mem in "${mem_configs[@]}"
            do
                slab_mem=$((mem * 1024 * 1024 * 1024))
                prefix=rpcperf_${conn}_${size}_${mem}
                python client_config.py --prefix="$prefix" --server_ip="$target" --instances="$instances" --rate="$rate" --connections="$conn" --vsize "$vsize" --slab_mem="$slab_mem" --threads="$threads"
            done
        done
    done
}

get_args "${@}"
if [ "$client" = true ]; then
    gen_rpcperf
fi
if [ "$server" = true ]; then
    gen_pelikan
fi


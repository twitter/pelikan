
# per pelikan instance
export conn_configs=(100 500 1000 2000 5000 10000)
#export mem_bytes=$((8 * 1024 * 1024 * 1024))
export mem_bytes=$((4 * 1024 * 1024 * 1024))
#export size_configs=(64 256 1024 4096 16384)
export size_configs=(64 4096)
export thrpt_configs=(0.5 2) # M QPS
export pelikan_instances=24
export rpcperf_instances=24   # per host
export ksize=32
export get_weight=9
export set_weight=1
export rpcperf_threads=2

export pmem_paths=()
export use_adq=1

export SERVER=smf1-ifm-21-sr1
export CLIENTS=(smf1-dwk-03-sr1 smf1-dwk-27-sr1 smf1-dwk-31-sr1 smf1-hii-17-sr1 smf1-dwk-24-sr1 smf1-dwk-26-sr1 smf1-dtk-30-sr1 smf1-dtk-21-sr1)

#export SERVER=smf1-iio-03-sr1
#export SERVER=smf1-dwk-27-sr1


export cache_spare=(smf1-hgn-23-sr1 smf1-hhn-06-sr1 smf1-hkh-10-sr1 smf1-hog-18-sr1 smf1-hok-25-sr1 smf1-hok-31-sr1 smf1-hol-16-sr1 smf1-hol-20-sr1 smf1-hon-25-sr1 smf1-hoo-20-sr1 smf1-hoq-21-sr1 smf1-hos-10-sr1 smf1-hos-28-sr1 smf1-hot-19-sr1 smf1-hot-20-sr1 smf1-hot-29-sr1 smf1-how-22-sr1 smf1-hzq-04-sr1 smf1-hzq-10-sr1 smf1-hzz-32-sr1)
export CLIENTS=("${CLIENTS[@]}" "${cache_spare[@]}")

# Initialize our own variables:
#export client_config=""
#export server_config=""
#export clients=""
export pelikan_binary="$HOME/pelikan_twemcache"
export rpcperf_binary="$HOME/rpc-perf"


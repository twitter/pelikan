
# per pelikan instance
export conn_configs=(100 500 1000 2000 5000 10000)
export mem_bytes=$((4 * 1024 * 1024 * 1024))
export size_configs=(64 4096)
export thrpt_configs=(0.5 1 2) # M QPS
export pelikan_instances=24
# export rpcperf_instances=24   # per host
export rpcperf_instances=1   # per host
export ksize=32
export get_weight=9
export set_weight=1
export rpcperf_threads=20

export pmem_paths=()
export use_adq=1

export SERVER=
export CLIENTS=()


export pelikan_binary="$HOME/pelikan_twemcache"
export rpcperf_binary="$HOME/rpc-perf"

rm hosts 2>/dev/null
for c in ${CLIENTS[@]}; do
  echo $c >> hosts;
done

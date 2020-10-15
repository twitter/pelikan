#!/bin/bash

source params.sh > /dev/null

show_help() {
  echo 'generate.sh [-c [-r absolute/path/to/rpcperf] [-t server_ip]] [-s [-p absolute/path/to/pelikan]] [-m "path/to/pmem0 path/to/pmem1 ..."]'
  echo 'Note that the first pmem path is bound to the first numa node, the second path is bound to the next numa node. One or more paths can be provided.'
}

get_args() {
  while getopts ":p:r:t:m:csh" opt; do
    case "$opt" in
    c)
      client=true
      ;;
    s)
      server=true
      ;;
    p)
      pelikan=$OPTARG
      ;;
    r)
      rpcperf=$OPTARG
      ;;
    t)
      server_ip=$OPTARG
      ;;
    m)
      pmem_paths=($OPTARG)
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
gen_pelikan() {
  for size in "${size_configs[@]}"; do
    vsize=$((size - ksize))
    prefix=pelikan_${size}
    python3 server_config.py --prefix="$prefix" --binary="$pelikan_binary" --instances="$pelikan_instances" --mem_bytes "$mem_bytes" --ksize "$ksize" --vsize "$vsize" --use_adq $use_adq --worker_binding True --pmem_paths ${pmem_paths[@]}
  done
}

# rpc-perf configs
gen_rpcperf() {
  backend=${pelikan_binary##*_}
  for thrpt in "${thrpt_configs[@]}"; do
    nhost=${#CLIENTS[@]}
    rate=$(echo "${thrpt} * 1000000 / ${rpcperf_instances} / ${nhost}" | bc)
    for conn in "${conn_configs[@]}"; do
      conn_per_instance=$conn
      conn_per_instance=$(echo "${conn} * ${pelikan_instances} / ${rpcperf_instances} / ${nhost}" | bc)
#      conn_per_instance=$(echo "$conn / ${nhost} + 0.5" | bc -l)
#      conn_per_instance=${conn_per_instance%.*}
      for size in "${size_configs[@]}"; do
        vsize=$((size - ksize))
        prefix=rpcperf_${thrpt}_${conn}_${size}
        python3 client_config.py --prefix="$prefix" --backend="$backend" --binary="$rpcperf_binary" --server_ip="$server_ip" --instances="$rpcperf_instances" --rate="$rate" --connections="$conn_per_instance" --ksize="$ksize" --vsize "$vsize" --mem_bytes="$mem_bytes" --get_weight="${get_weight}" --set_weight="${set_weight}" --threads="${rpcperf_threads}"
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



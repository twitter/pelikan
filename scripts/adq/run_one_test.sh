#!/bin/bash

source params.sh > /dev/null

show_help() {
  echo "runtest.sh -c <client_config_path> -s <server_config_path> -m <\"one or more client ips\">"
}

get_args() {
  while getopts ":c:s:m:h" opt; do
    case "$opt" in
    c)
      client_config=$OPTARG
      ;;
    s)
      server_config=$OPTARG
      ;;
    m)
      clients=($OPTARG)
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

server_launch() {
  init_dir=$(pwd)
  folder=${server_config##*/}

  sudo pkill -f ${pelikan_binary} 2>/dev/null
  rm -rf "/tmp/${folder}" 2>/dev/null

  cp -r "${server_config}" /tmp/
  cd "/tmp/$folder" && ./warm-up.sh
  cd "${init_dir}" || exit 1
}

client_run() {
  folder=${client_config##*/}
  nClient=${#CLIENTS[@]}
  for i in $(seq 0 $((nClient-1))); do
    client=${CLIENTS[$i]}
    sleep 0.5
    (
      scp -rq "${client_config}" "$client:/tmp/"
      port=$(echo "12300+ (${i} % ${pelikan_instances})" | bc)
      echo client "$i $client - $client_config" port $port
      ssh -q "$client" -tt "cd /tmp/${folder} && sed -i 's/12300/${port}/g' test.sh && ./test.sh; "
      scp -q "${client}:/tmp/${folder}/rpcperf_${port}_0.log" rpcperf_log/${folder}_${port}_0.log.${client}
#      for i in $(seq 0 $((rpcperf_instances-1))); do
#        port=$((12300 + i))
#        scp -q "${client}:/tmp/${folder}/rpcperf_${port}_${i}.log" rpcperf_log/${folder}_${port}_${i}.log.${client}
#      done
    ) &
  done
  wait
}

cleanup() {
  sudo pkill -f ${pelikan_binary}
}


trap "cleanup; exit" SIGHUP SIGINT SIGTERM

get_args "${@}"
server_launch
client_run
cleanup

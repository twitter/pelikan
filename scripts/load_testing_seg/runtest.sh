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
  for client in ${CLIENTS[@]}; do
    echo client "$client - $client_config"
    (
      scp -rq "${client_config}" "$client:/tmp/"
      ssh "$client" -tt "cd /tmp/${folder} && ./test.sh; "
      for i in `seq 0 $((rpcperf_instances-1))`; do
        port=$((12300 + i))
        scp -q "${client}:/tmp/${folder}/rpcperf_${port}_${i}.log" rpcperf_log/${folder}_${port}_${i}.log.${client}
      done
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

#!/bin/bash

# client is remote or localhost
# server is localhost


source params.sh > /dev/null
trap "echo ${pelikan_binary##*/}; sudo pkill ${pelikan_binary##*/}; exit" SIGHUP SIGINT SIGTERM

setup()
{
  sudo rm -r /tmp/pelikan* 2>/dev/null
  sudo rm -r rpcperf_* pelikan_* 2>/dev/null
  mkdir rpcperf_log 2>/dev/null;
  echo "${#CLIENTS[@]} clients"
  for client in ${CLIENTS[@]}; do
    (
    echo $client
    pkill rpc-perf
    ssh -q -o "StrictHostKeyChecking=no" "$client" -tt "pkill -9 rpc-perf; rm -rf /tmp/rpcperf*; sleep 2"
    scp -q -o "StrictHostKeyChecking=no" "${rpcperf_binary}" "$client:"
    )
  done
  wait
}

gen_conf()
{
  ./generate.sh -c -r $HOME/rpc-perf -s -p $HOME/pelikan_twemcache -t "${SERVER}"
}

run_all_tests()
{
  for server_conf in pelikan_*; do
    item_size=${server_conf##*_}
    for client_conf in rpcperf_*_"${item_size}"; do
      echo -e "####### start ${server_conf} \t----- ${client_conf} \t#######"
#      if [ -f "rpcperf_log/${client_conf}_12000_0.log.smf1-hii-17-sr1" ]; then
#        continue
#      fi
      ./runtest.sh -s "${server_conf}" -c "${client_conf}" -m "${CLIENTS}"
    done
  done
  tar cvf rpcperf_log.tar.gz rpcperf_log
}


setup
gen_conf
run_all_tests

tput bel
date

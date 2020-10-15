#!/bin/bash

# client is remote or localhost
# server is localhost


source params.sh > /dev/null
trap "echo ${pelikan_binary##*/}; sudo pkill ${pelikan_binary##*/}; exit" SIGHUP SIGINT SIGTERM

setup()
{
  sudo rm -r /tmp/pelikan* 2>/dev/null
  sudo rm -r rpcperf_* pelikan_* 2>/dev/null
  sudo pkill -9 -f pelikan
  mkdir rpcperf_log 2>/dev/null;
  echo "${#CLIENTS[@]} clients"
  pssh -O StrictHostKeyChecking=no -h hosts "pkill -9 rpc-perf 2>/dev/null; rm -rf /tmp/rpcperf* 2>/dev/null; sleep 2"
  pscp.pssh -h hosts "${rpcperf_binary}" $HOME/;
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
      if [ -f "rpcperf_log/${client_conf}_12300_0.log.smf1-dwk-24-sr1" ]; then
        echo skip ${client_conf}
        continue
      fi
      ./run_one_test.sh -s "${server_conf}" -c "${client_conf}" -m "${CLIENTS}"
    sleep 80
    done
  done
  tar cvf rpcperf_log.tar.gz rpcperf_log
}


setup
gen_conf
run_all_tests

tput bel
date


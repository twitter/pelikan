#!/bin/bash


# pmem_path=/dev/dax0.0
pmem_path=/mnt/pmem1/pelikan.map
# pmem_path=


compile()
{
  d=$(pwd)
  if [ "$pmem_path" != "" ]; then
    echo PMem enabled
    cd ../../_build && rm -r * && cmake3 -DHAVE_ASSERT_LOG=off -DHAVE_ASSERT_PANIC=off -DHAVE_LOGGING=off -DHAVE_STATS=off -DHAVE_TEST=off -DHAVE_DEBUG_MM=off -DCMAKE_BUILD_TYPE=Release -DUSE_PMEM=on .. && make -j && cd -
  else
    cd ../../_build && rm -r * && cmake3 -DHAVE_ASSERT_LOG=off -DHAVE_ASSERT_PANIC=off -DHAVE_LOGGING=off -DHAVE_STATS=off -DHAVE_TEST=off -DHAVE_DEBUG_MM=off -DCMAKE_BUILD_TYPE=Release -DUSE_PMEM=off .. && make -j && cd -
  fi
}

setup()
{

  compile

  cp ../../_build/benchmarks/trace_replay_slab .
  cp ../../_build/benchmarks/trace_replay_seg .

  mkdir log 2>/dev/null
}


runMT()
{
  for bench_binary in trace_replay_seg trace_replay_slab; do
    for n_thread in 1 2 4 8 12 16 20; do
      for j in `seq 1 ${n_thread}`; do
        echo -e "debug_logging: no
debug_log_level: 4
trace_path: /home/junchengy/tweetypie_cache.sbin
default_ttl_list: 86400:1
heap_mem: 2048576000
hash_power: 26
evict_opt: 1
n_thread:1" > bench.conf.${j}

        if [ "$pmem_path" != "" ]; then
          echo -e "datapool_path: ${pmem_path}.${j}\ndatapool_name: pmem0\ndatapool_prefault: no" >> bench.conf.${j}
        fi

        taskset -c $j ./${bench_binary} bench.conf.${j} | tee -a thrpt_bench_${n_thread}.log &
      done
      wait
      echo '##############################################' >> thrpt_bench_${n_thread}.log
    done
  done
}


setup
#run
runMT
tput bel
date

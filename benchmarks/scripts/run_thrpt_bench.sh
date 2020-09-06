#!/bin/bash

op_names=(get gets set add cas replace append prepend delete incr decr)

bench_binaries=(bench_slab_thrpt bench_seg_thrpt)
entry_sizes=(64 4096)
nentries=(100663296 6291456)
nops=(100663296 6291456)

nentries=(400663296 16291456)
nops=(400663296 16291456)

#nentries=(10066329 2291456)
#nops=(10066329 2291456)

# pmem_path=/dev/dax0.0
pmem_path=/mnt/pmem1/pelikan.map
# pmem_path=


setup()
{
  cp ../../_build/benchmarks/bench_slab_thrpt .
  cp ../../_build/benchmarks/bench_seg_thrpt .

  if [ "$pmem_path" != "" ]; then
    echo PMem enabled
  fi
}


run() {
  n_entry_sizes=${#entry_sizes[@]}
  n_nentries=${#nentries[@]}
  if [[ ${n_entry_sizes} != ${n_nentries} ]]; then
    echo "len of entry_sizes and len of nentries do not match, exit"
    exit
  fi

  end=$((n_entry_sizes-1))
  for i in `seq 0 $end`; do
    entry_size=${entry_sizes[i]}
    nentry=${nentries[i]}
    nop=${nops[i]}

#    for op in 0 2 4 8 9; do
    for op in 0 2; do
      for bench_binary in ${bench_binaries[@]}; do

        echo -e "entry_size: ${entry_size}
nentries: ${nentry}
nops: ${nop}
debug_log_level: 4
debug_logging: no
op: ${op}" > bench.conf

        if [ "$pmem_path" != "" ]; then
          echo -e "datapool_path: ${pmem_path}\ndatapool_name: pmem0\ndatapool_prefault: no" >> bench.conf
        fi

        taskset -c 0 ./${bench_binary} bench.conf | tee -a thrpt_bench.log
      done
    done
  done
}

runMT()
{
  n_entry_sizes=${#entry_sizes[@]}
  n_nentries=${#nentries[@]}
  if [[ ${n_entry_sizes} != ${n_nentries} ]]; then
    echo "len of entry_sizes and len of nentries do not match, exit"
    exit
  fi

  end=$((n_entry_sizes-1))
  for n_thread in 1 2 4 8 12 16 20; do
    for i in `seq 0 $end`; do
      entry_size=${entry_sizes[i]}
      nentry=$((nentries[i]/n_thread))
      nop=$((nops[i]/n_thread))

      for op in 0 2; do
        for bench_binary in ${bench_binaries[@]}; do
          for j in `seq 1 ${n_thread}`; do

            echo -e "entry_size: ${entry_size}
nentries: ${nentry}
nops: ${nop}
debug_log_level: 4
debug_logging: no
op: ${op}" > bench.conf.${j}

            if [ "$pmem_path" != "" ]; then
              echo -e "datapool_path: ${pmem_path}.${j}\ndatapool_name: pmem0\ndatapool_prefault: no" >> bench.conf.${j}
            fi

            taskset -c $j ./${bench_binary} bench.conf.${j} | tee -a thrpt_bench_${n_thread}.log &
          done
          wait
          echo '##############################################' >> thrpt_bench_${n_thread}.log
        done
      done
    done
  done
}


setup
#run
runMT
tput bel
date

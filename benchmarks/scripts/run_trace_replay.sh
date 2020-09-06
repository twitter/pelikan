#!/bin/bash

sizes=(1073741824 2147483648 8589934592 4294967296 17179869184)
sizes=(104857600 1073741824)
# TTL media_metadata
# write-heavy prediction_from_ranking
# small working set ads_merged_counting_features
traces=(gizmoduck tweetypie strato_negative_result livepipeline simclusters_v2_entity_cluster_scores graph_feature_service geouser passbird content_recommender pinkfloyd blender_adaptive search_roots talon)
trace_base_path=/mnt/nfs/twem/
trace_base_path=/data/twoDay/

# trace_path=$HOME/tweetypie.bin.processed.2


# pmem_path=/dev/dax0.0
# pmem_path=/data/pool1/pelikan.map
pmem_path=


compile()
{
  d=$(pwd)
#  cd ../../src/storage/seg/ && sed -i "s/cc_memcpy(item_val(it), val->data, val->len)//g" item.c && cd -
#  cd ../../src/storage/slab/ && sed -i "s/cc_memcpy(item_val(it), val->data, val->len)//g" item.c && cd -
  cd ../../_build && make -j && cd ${d}
}

setup()
{
  if [ "$pmem_path" != "" ]; then
    echo PMem enabled
  fi

  killall -9 trace_replay_seg
  killall -9 trace_replay_slab

  compile

  cp ../../_build/benchmarks/trace_replay_slab .
  cp ../../_build/benchmarks/trace_replay_seg .
  cp -r ../config/trace_conf .

  mkdir log 2>/dev/null
}

run_slab() {
  trace=$1
  size=$2
  cp trace_conf/${trace}.conf trace_conf/slab_${trace}_${size}.conf;
  echo -e "trace_path: ${trace_base_path}/${trace}_cache.sbin
heap_mem: ${size}
hash_power: 28
evict_opt: 1" >> trace_conf/slab_${trace}_${size}.conf;

    if [ "$pmem_path" != "" ]; then
        echo -e "datapool_path: ${pmem_path}\ndatapool_name: pmem0\ndatapool_prefault: yes" >> trace_conf/slab_${trace}_${size}.conf
    fi

    ./trace_replay_slab trace_conf/slab_${trace}_${size}.conf
}

run_seg()
{
  trace=$1
  size=$2
  cp trace_conf/${trace}.conf trace_conf/seg_${trace}_${size}.conf
  echo -e "trace_path: ${trace_base_path}/${trace}_cache.sbin
heap_mem: ${size}
hash_power: 28
evict_opt: 1
" >> trace_conf/seg_${trace}_${size}.conf

  if [ "$pmem_path" != "" ]; then
    echo -e "datapool_path: ${pmem_path}\ndatapool_name: pmem0\ndatapool_prefault: yes" >> trace_conf/seg_${trace}_${size}.conf
  fi

  ./trace_replay_seg trace_conf/seg_${trace}_${size}.conf
}

run_explore()
{
  mkdir log 2>/dev/null
  for trace in ${traces[@]}; do
    for size in ${sizes[@]}; do
      run_slab ${trace} ${size} > log/slab_${trace}_${size} &
      run_seg ${trace} ${size} > log/seg_${trace}_${size} &
    done
  done
  wait
}

run_conf()
{
  for t in small medium large; do
    for f in trace_conf/${t}/*; do
      trace=${f##*/}
      trace=${trace%%.*}
      echo trace ${trace}
      echo -e "\ntrace_path: ${trace_base_path}/${trace}_cache.sbin" >> $f
      ./trace_replay_slab $f | tee log/${trace}_${t}_slab &
      ./trace_replay_seg $f | tee log/${trace}_${t}_seg_merge &
    done
  done
  wait
}


run_conf2()
{
  for f in trace_conf/selected/*; do
    trace0=${f##*/}
    trace=${trace0%%.*}
    echo trace ${trace}
    echo -e "\ntrace_path: ${trace_base_path}/${trace}_cache.sbin" >> $f
    # ./trace_replay_slab $f | tee log/${trace}_${trace0}_slab &
    ./trace_replay_seg $f | tee log/${trace}_${trace0}_seg_merge &
    sleep 2
  done
  wait
}

setup
# run_explore
run_conf2

tput bel
date

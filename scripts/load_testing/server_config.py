import argparse
from math import ceil, floor, log
import os

INSTANCES = 3
PREFIX = 'test'
PELIKAN_ADMIN_PORT = 9900
PELIKAN_SERVER_PORT = 12300
PELIKAN_SLAB_MEM = 4294967296
PELIKAN_ITEM_OVERHEAD = 48
KSIZE = 32
VSIZE = 32
THREAD_PER_SOCKET = 48
BIND_TO_CORES = False
BIND_TO_NODES = True

def generate_config(instances, vsize, slab_mem):
  # create top-level folders under prefix
  try:
    os.makedirs('config')
  except:
    pass
  try:
    os.makedirs('log')
  except:
    pass

  nkey = int(ceil(1.0 * slab_mem / (vsize + KSIZE + PELIKAN_ITEM_OVERHEAD)))
  hash_power = int(ceil(log(nkey, 2)))

  # create twemcache config file(s)
  for i in range(instances):
    admin_port = PELIKAN_ADMIN_PORT + i
    server_port = PELIKAN_SERVER_PORT + i
    config_file = 'twemcache-{server_port}.config'.format(server_port=server_port)
    config_str = """\
daemonize: yes
admin_port: {admin_port}
server_port: {server_port}

admin_tw_cap: 2000

buf_init_size: 4096

buf_sock_poolsize: 16384

debug_log_level: 5
debug_log_file: log/twemcache-{server_port}.log
debug_log_nbuf: 1048576

klog_file: log/twemcache-{server_port}.cmd
klog_backup: log/twemcache-{server_port}.cmd.old
klog_sample: 100
klog_max: 1073741824

prefill: yes
prefill_ksize: 32
prefill_vsize: {vsize}
prefill_nkey: {nkey}

request_poolsize: 16384
response_poolsize: 32768

slab_evict_opt: 1
slab_prealloc: yes
slab_hash_power: {hash_power}
slab_mem: {slab_mem}
slab_size: 1048756

stats_intvl: 10000
stats_log_file: log/twemcache-{server_port}.stats

time_type: 2
""".format(admin_port=admin_port, server_port=server_port, vsize=vsize, nkey=nkey, hash_power=hash_power, slab_mem=slab_mem)
    with open(os.path.join('config', config_file),'w') as the_file:
      the_file.write(config_str)

def generate_runscript(binary, instances):
  # create bring-up.sh
  fname = 'bring-up.sh'
  with open(fname, 'w') as the_file:
    for i in range(instances):
      config_file = os.path.join('config', 'twemcache-{server_port}.config'.format(server_port=PELIKAN_SERVER_PORT+i))
      if BIND_TO_NODES:
        the_file.write('sudo numactl --cpunodebind={numa_node} --preferred={numa_node} '.format(
            numa_node=i%2))
      elif BIND_TO_CORES:
        the_file.write('sudo numactl --physcpubind={physical_thread},{logical_thread} '.format(
            physical_thread=i,
            logical_thread=i+THREAD_PER_SOCKET))
      the_file.write('{binary_file} {config_file}\n'.format(
          binary_file=binary,
          config_file=config_file))
  os.chmod(fname, 0777)

  # create warm-up.sh
  fname = 'warm-up.sh'
  with open(fname, 'w') as the_file:
    the_file.write("""
./bring-up.sh

nready=0
while [ $nready -lt {instances} ]
do
    nready=$(grep -l "prefilling slab" log/twemcache-*.log | wc -l)
    echo "$(date): $nready out of {instances} servers are warmed up"
    sleep 10
done
""".format(instances=instances))
  os.chmod(fname, 0777)


if __name__ == "__main__":
  parser = argparse.ArgumentParser(description="""
    Generate all the server-side scripts/configs needed for a test run.
    """)
  parser.add_argument('--binary', dest='binary', type=str, help='location of pelikan_twemcache binary', required=True)
  parser.add_argument('--prefix', dest='prefix', type=str, default=PREFIX, help='folder that contains all the other files to be generated')
  parser.add_argument('--instances', dest='instances', type=int, default=INSTANCES, help='number of instances')
  parser.add_argument('--vsize', dest='vsize', type=int, default=VSIZE, help='value size')
  parser.add_argument('--slab_mem', dest='slab_mem', type=int, default=PELIKAN_SLAB_MEM, help='total capacity of slab memory, in bytes')

  args = parser.parse_args()

  if not os.path.exists(args.prefix):
    os.makedirs(args.prefix)
  os.chdir(args.prefix)

  generate_config(args.instances, args.vsize, args.slab_mem)
  generate_runscript(args.binary, args.instances)

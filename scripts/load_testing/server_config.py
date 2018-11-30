import os
import argparse

INSTANCES = 3
PREFIX = 'test'
PELIKAN_ADMIN_PORT = 9900
PELIKAN_SERVER_PORT = 12300
PELIKAN_SERVER_IP = '10.25.2.45'
PELIKAN_SLAB_MEM = 4294967296
PELIKAN_BINARY = '/root/Twitter/pelikan/_build/_bin/pelikan_twemcache'
THREAD_PER_SOCKET = 48
BIND_TO_CORES = False
BIND_TO_NODES = True

def generate_config(prefix, instances, slab_mem):
  # create top-level folders under prefix
  config_path = os.path.join(prefix, 'config')
  os.makedirs(config_path)
  log_path = os.path.join(prefix, 'log')
  os.makedirs(log_path)

  # create twemcache config file(s)
  for i in range(instances):
    admin_port = PELIKAN_ADMIN_PORT + i
    server_port = PELIKAN_SERVER_PORT + i
    config_file = 'pelikan-{server_port}.config'.format(server_port=server_port)
    config_str = """\
daemonize: yes
admin_port: {admin_port}
server_port: {server_port}

buf_init_size: 4096

buf_sock_poolsize: 16384

debug_log_level: 5
debug_log_file: log/{server_port}/twemcache.log
debug_log_nbuf: 1048576

klog_file: log/{server_port}/twemcache.cmd
klog_backup: log/{server_port}/twemcache.cmd.old
klog_sample: 100
klog_max: 1073741824

request_poolsize: 16384
response_poolsize: 32768

slab_evict_opt: 1
slab_prealloc: yes
slab_hash_power: 26
slab_mem: {slab_mem}
slab_size: 1048756
""".format(admin_port=admin_port, server_port=server_port, slab_mem=slab_mem)
    try:
      os.makedirs(os.path.join(log_path, str(server_port)))
    except:
      pass
    with open(os.path.join(config_path, config_file),'w') as the_file:
      the_file.write(config_str)

def generate_runscript(prefix, instances):
  config_path = os.path.join(prefix, 'config')
  # create bring-up.sh
  with open('bring-up.sh','w') as the_file:
    for i in range(instances):
      config_file = os.path.join(config_path, 'pelikan-{server_port}.config'.format(server_port=PELIKAN_SERVER_PORT+i))
      if BIND_TO_NODES:
        the_file.write('sudo numactl --cpunodebind={numa_node} --preferred={numa_node} '.format(
            numa_node=i%2))
      elif BIND_TO_CORES:
        the_file.write('sudo numactl --physcpubind={physical_thread},{logical_thread} '.format(
            physical_thread=i,
            logical_thread=i+THREAD_PER_SOCKET))
      the_file.write('{binary_file} {config_file}\n'.format(
          binary_file=PELIKAN_BINARY,
          config_file=config_file))
  os.chmod('bring-up.sh', 0777)

if __name__ == "__main__":
  parser = argparse.ArgumentParser(description="""
    Generate all the server-side scripts/configs needed for a test run.
    """)
  parser.add_argument('--prefix', dest='prefix', type=str, default=PREFIX, help='folder that contains all the other files to be generated')
  parser.add_argument('--instances', dest='instances', type=int, default=INSTANCES, help='number of instances')
  parser.add_argument('--slab_mem', dest='slab_mem', type=int, default=PELIKAN_SLAB_MEM, help='total capacity of slab memory, in bytes')

  args = parser.parse_args()

  generate_config(args.prefix, args.instances, args.slab_mem)
  generate_runscript(args.prefix, args.instances)

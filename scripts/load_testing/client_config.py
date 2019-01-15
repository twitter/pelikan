import argparse
from math import ceil
import os


INSTANCES = 3
PREFIX = 'loadgen'
RPCPERF_THREADS = 1
RPCPERF_CONNS = 100
RPCPERF_RATE = 10000
RPCPERF_GET_WEIGHT = 9
RPCPERF_SET_WEIGHT = 1
PELIKAN_SLAB_MEM = 4294967296
PELIKAN_ITEM_OVERHEAD = 48
KSIZE = 32
VSIZE = 32
PELIKAN_SERVER_PORT = 12300


def generate_config(rate, connections, vsize, slab_mem, threads):
# create rpcperf.toml
  nkey = int(ceil(1.0 * slab_mem / (vsize + KSIZE + PELIKAN_ITEM_OVERHEAD)))
  conn_per_thread = connections / threads

  config_str = '''
[general]
clients = {threads}
tcp-nodelay = true
poolsize = {connections} # this specifies number of connection per thread
# runtime ~= windows x duration
windows = 2
interval = 60
request_ratelimit = {rate}

[[keyspace]]
length = {ksize}
count = {nkey}
weight = 1
commands = [
    {{action = "get", weight = {get_weight}}},
    {{action = "set", weight = {set_weight}}},
]
values = [
    {{length = {vsize}, weight = 1}},
]'''.format(threads=threads, connections=conn_per_thread, nkey=nkey, rate=rate,
    ksize=KSIZE, vsize=vsize, get_weight=RPCPERF_GET_WEIGHT, set_weight=RPCPERF_SET_WEIGHT)

  with open('rpcperf.toml', 'w') as the_file:
    the_file.write(config_str)


def generate_runscript(binary, server_ip, instances):
  # create test.sh
  fname = 'test.sh'
  with open(fname, 'w') as the_file:
    for i in range(instances):
      server_port = PELIKAN_SERVER_PORT + i
      the_file.write('{binary_file} --config {config_file}'.format(binary_file=binary, config_file='rpcperf.toml'))
      the_file.write(' --server {server_ip}:{server_port}'.format(server_ip=server_ip, server_port=server_port))
      # Currently rpc-perf (prerelease) doesn't support waterfall,
      # it will be added back soon at which point we can turn this back on
      # the_file.write(' --waterfall latency-waterfall-{server_port}.png'.format(server_port=server_port))
      the_file.write(' > rpcperf_{server_port}.log'.format(server_port=server_port))
      the_file.write(' 2>&1 &\n')
  os.chmod(fname, 0777)


if __name__ == "__main__":
  parser = argparse.ArgumentParser(description="""
    Generate all the client-side scripts/configs needed for a test run.
    """)
  parser.add_argument('--binary', dest='binary', type=str, help='location of rpc-perf binary', required=True)
  parser.add_argument('--prefix', dest='prefix', type=str, default=PREFIX, help='folder that contains all the other files to be generated')
  parser.add_argument('--instances', dest='instances', type=int, default=INSTANCES, help='number of instances')
  parser.add_argument('--server_ip', dest='server_ip', type=str, help='server ip', required=True)
  parser.add_argument('--rate', dest='rate', type=int, default=RPCPERF_RATE, help='request rate per instance')
  parser.add_argument('--connections', dest='connections', type=int, default=RPCPERF_CONNS, help='number of connections per instance')
  parser.add_argument('--vsize', dest='vsize', type=int, default=VSIZE, help='value size')
  parser.add_argument('--slab_mem', dest='slab_mem', type=int, default=PELIKAN_SLAB_MEM, help='slab memory')
  parser.add_argument('--threads', dest='threads', type=int, default=RPCPERF_THREADS, help='number of worker threads per rpc-perf')

  args = parser.parse_args()

  if not os.path.exists(args.prefix):
    os.makedirs(args.prefix)
  os.chdir(args.prefix)

  generate_config(args.rate, args.connections, args.vsize, args.slab_mem, args.threads)
  generate_runscript(args.binary, args.server_ip, args.instances)

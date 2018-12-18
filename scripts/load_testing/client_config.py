import argparse
from math import ceil
import os


INSTANCES = 3
PREFIX = 'loadgen'
RPCPERF_THREADS = 1
RPCPERF_CONNS = 100
RPCPERF_RATE = 10000
RPCPERF_GET_RATIO = 0.9
RPCPERF_SET_RATIO = 1 - RPCPERF_GET_RATIO
PELIKAN_SLAB_MEM = 4294967296
PELIKAN_ITEM_OVERHEAD = 48
KSIZE = 32
VSIZE = 32
PELIKAN_SERVER_PORT = 12300
PELIKAN_SERVER_IP = '10.25.2.44'
RPCPERF_BINARY = '/root/Twitter/rpc-perf/target/release/rpc-perf'


def generate_config(rate, connections, vsize, slab_mem, threads):
# create rpcperf.toml
  rate_get = int(rate * RPCPERF_GET_RATIO)
  rate_set = int(rate * RPCPERF_SET_RATIO)
  nkey = int(ceil(1.0 * slab_mem / (vsize + KSIZE + PELIKAN_ITEM_OVERHEAD)))
  conn_per_thread = connections / threads

  config_str = """\
[general]
threads = {threads}
tcp-nodelay = true
connections = {connections}
windows = 600
duration = 60
request-timeout = 200
connect-timeout = 50

[[workload]]
name = "get"
method = "get"
rate = {rate_get}
  [[workload.parameter]]
  style = "random"
  size = 32
  num = {nkey}
  regenerate = true

[[workload]]
name = "set"
method = "set"
rate = {rate_set}
  [[workload.parameter]]
  style = "random"
  size = 32
  num = {nkey}
  regenerate = true
  [[workload.parameter]]
  style = "random"
  size = {vsize}
  regenerate = false
""".format(threads=threads, connections=conn_per_thread, nkey=nkey, rate_get=rate_get, rate_set=rate_set, vsize=vsize)
  with open('rpcperf.toml', 'w') as the_file:
    the_file.write(config_str)


def generate_runscript(instances):
  # create test.sh
  fname = 'test.sh'
  with open(fname, 'w') as the_file:
    for i in range(instances):
      server_port = PELIKAN_SERVER_PORT + i
      the_file.write('{binary_file} --config {config_file}'.format(binary_file=RPCPERF_BINARY, config_file='rpcperf.toml'))
      the_file.write(' --server {server_ip}:{server_port} &\n'.format(server_ip=PELIKAN_SERVER_IP, server_port=server_port))
  os.chmod(fname, 0777)


if __name__ == "__main__":
  parser = argparse.ArgumentParser(description="""
    Generate all the client-side scripts/configs needed for a test run.
    """)
  parser.add_argument('--prefix', dest='prefix', type=str, default=PREFIX, help='folder that contains all the other files to be generated')
  parser.add_argument('--instances', dest='instances', type=int, default=INSTANCES, help='number of instances')
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
  generate_runscript(args.instances)

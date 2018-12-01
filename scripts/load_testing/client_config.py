import argparse
import os

INSTANCES = 3
PREFIX = 'test'
THREADS = 1
CONNECTIONS = 100
RATE = 10000
GET_RATIO = 0.9
SET_RATIO = 1 - GET_RATIO
SIZE = 4
PELIKAN_SERVER_PORT = 12300
PELIKAN_SERVER_IP = '10.25.2.44'
RPCPERF_BINARY = '/root/Twitter/rpc-perf/target/release/rpc-perf'

def generate_config(prefix, threads, connections, rate, size):
# create rpcperf.toml
  rate_get = int(rate * GET_RATIO)
  rate_set = int(rate * SET_RATIO)
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
  size = {size}
  regenerate = true

[[workload]]
name = "set"
method = "set"
rate = {rate_set}
  [[workload.parameter]]
  style = "random"
  size = {size}
  regenerate = true
[[workload.parameter]]
  style = "random"
  size = {size}
  regenerate = false
  """.format(threads=threads, connections=connections, rate_get=rate_get, rate_set=rate_set, size=size)
  with open(os.path.join(prefix, 'rpcperf.toml'), 'w') as the_file:
    the_file.write(config_str)

def generate_runscript(prefix, instances):
# create test.sh
  fname = os.path.join(prefix, 'test.sh')
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
  parser.add_argument('--threads', dest='threads', type=int, default=THREADS, help='number of worker threads per rpc-perf')
  parser.add_argument('--connections', dest='connections', type=int, default=CONNECTIONS, help='number of connections PER THREAD')
  parser.add_argument('--rate', dest='rate', type=int, default=RATE, help='aggregated request rate')
  parser.add_argument('--size', dest='size', type=int, default=SIZE, help='payload size')

  args = parser.parse_args()

  if not os.path.exists(args.prefix):
    os.makedirs(args.prefix)

  generate_config(args.prefix, args.threads, args.connections, args.rate, args.size)
  generate_runscript(args.prefix, args.instances)

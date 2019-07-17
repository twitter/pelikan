from __future__ import print_function
import argparse
from math import ceil, floor, log
import textwrap


# constants: units
K = 1000
M = K * 1000
KB = 1024
MB = 1024 * KB
GB = 1024 * MB

# constants: defaults
DEFAULT_QPS = 100  # (K)
DEFAULT_NKEY = 100  # (M)
DEFAULT_NCONN = 5 * K
DEFAULT_FAILURE_DOMAIN = 5.0  # 5% of the nodes may be lost at once
DEFAULT_SIZE = 64  # slimcache only

MAX_HOST_LIMIT = 10  # based on platform / job size constraint

# constants: pelikan related
CONN_OVERHEAD = 33 * KB  # 2 16KiB buffers, one channel, and stream overhead
SAFETY_BUF = 128  # in MB
BASE_OVERHEAD = 10  # in MB
KQPS = 30  # much lower than single-instance max, picked to scale to 10 jobs/host
HASH_OVERHEAD = {'twemcache': 8, 'slimcache': 0}
ITEM_OVERHEAD = {'twemcache': 40 + 8, 'slimcache': 6 + 8}  # ITEM_HDR_SIZE + CAS
KEYVAL_ALIGNMENT = 8  # in bytes
NITEM_ALIGNMENT = 512  # so memory allocation is always 4K (page size) aligned

# constants: job related
CPU_PER_JOB = 2.0
DISK_PER_JOB = 3   # in GB
RAM_CANDIDATES = [4, 8]  # in GB
RACK_TO_HOST_RATIO = 2.0  # a somewhat arbitrary ratio between rack/host-limit
FAILURE_DOMAIN_LOWER = 0.5
FAILURE_DOMAIN_UPPER = 20.0
WARNING_THRESHOLD = 1000  # alert when too many jobs are needed


def hash_parameters(nkey, runnable):
  hash_power = int(ceil(log(nkey, 2)))
  ram_hash = int(ceil(1.0 * HASH_OVERHEAD[runnable] * (2 ** hash_power) / MB))
  return (hash_power, ram_hash)


def calculate(args):
  """calculate job configuration according to requirements.
     For twemcache, returns a dict with:
      cpu, ram, disk,
      hash_power, slab_mem,
      instance, host_limit, rack_limit,
      memory_bound
     For slimcache, return a dict with:
      cpu, ram, disk,
      item_size, nitem,
      instance, host_limit, rack_limit,
      memory_bound
  """
  if args.failure_domain < FAILURE_DOMAIN_LOWER or args.failure_domain > FAILURE_DOMAIN_UPPER:
    print('ERROR: failure domain should be between {:.1f}% and {:.1f}'.format(
      FAILURE_DOMAIN_LOWER, FAILURE_DOMAIN_UPPER))

  # first calculate njob disrecarding memory, note both njob & bottleneck are not yet final
  njob_qps = int(ceil(1.0 * args.qps / KQPS))
  njob_fd = int(ceil(100.0 / args.failure_domain))
  if njob_qps >= njob_fd:
    bottleneck = 'qps'
    njob = njob_qps
  else:
    bottleneck = 'failure domain'
    njob = njob_fd

  # then calculate njob (vector) assuming memory-bound

  # all ram-related values in this function are in MB
  # amount of ram needed to store dataset, factoring in overhead
  item_size = int(KEYVAL_ALIGNMENT * ceil(1.0 * (ITEM_OVERHEAD[args.runnable] + args.size) /
    KEYVAL_ALIGNMENT))
  ram_data = 1.0 * item_size * args.nkey * M / MB
  # per-job memory overhead, in MB
  ram_conn = int(ceil(1.0 * CONN_OVERHEAD * args.nconn / MB))
  ram_fixed = BASE_OVERHEAD + SAFETY_BUF

  njob_mem = []
  sorted_ram = sorted(args.ram)
  for ram in sorted_ram:
    ram = ram * GB / MB  # change unit to MB
    n_low = int(ceil(ram_data / ram))  # number of shards, lower bound
    nkey_per_shard = 1.0 * args.nkey * M / n_low  # number of keys per shard, upper bound
    hash_power, ram_hash = hash_parameters(nkey_per_shard, args.runnable)  # upper bound for both
    n = int(ceil(ram_data / (ram - ram_fixed - ram_conn - ram_hash)))
    njob_mem.append(n)

  # get final njob count; prefer larger ram if it reduces njob, which means:
  # if cluster needs higher job ram AND more instances due to memory, update njob
  # if cluster is memory-bound with smaller job ram but qps-bound with larger ones, use higher ram
  # otherwise, use smaller job ram and keep njob value unchanged
  index = 0  # if qps bound, use smallest ram setting
  for i, n in reversed(list(enumerate(njob_mem))[1:]):
    if n > njob or njob_mem[i - 1] > njob:
      bottleneck = 'memory'
      index = i
      njob = max(njob, n)
      break
  if njob > WARNING_THRESHOLD:
    print('WARNING: more than {} instances needed, please verify input.'.format(WARNING_THRESHOLD))

  # recalculate hash parameters with the final job count
  nkey_per_shard = 1.0 * (sorted_ram[index] * GB - ram_fixed * MB - ram_conn * MB) / item_size
  # only used by twemcache
  hash_power, ram_hash = hash_parameters(nkey_per_shard, args.runnable)
  slab_mem = sorted_ram[index] * GB / MB - ram_fixed - ram_conn - ram_hash
  # only used by slimcache
  nitem = int(NITEM_ALIGNMENT * floor(nkey_per_shard / NITEM_ALIGNMENT))

  rack_limit = int(floor(njob * args.failure_domain / 100))  # >= 1 given how we calculate njob
  host_limit = int(floor(min(MAX_HOST_LIMIT, max(1, rack_limit / RACK_TO_HOST_RATIO))))

  ret = {
      'cpu': CPU_PER_JOB,
      'ram': sorted_ram[index],
      'disk': DISK_PER_JOB,
      'instance': njob,
      'rack_limit': rack_limit,
      'host_limit': host_limit,
      'bottleneck': bottleneck}
  if args.runnable == 'twemcache':
    ret['hash_power'] = hash_power
    ret['slab_mem'] = slab_mem
  elif args.runnable == 'slimcache':
    ret['item_size'] = item_size
    ret['nitem'] = nitem

  return ret


def format_input(args):
  return textwrap.dedent('''
    Requirement:
      qps:             {} K
      key-val size:    {}
      number of key:   {} M
      data, computed:  {:.1f} GB
      number of conn:  {} per server
      failure domain:  {:.1f} %

  '''.format(args.qps, args.size, args.nkey,
             1.0 * args.size * args.nkey * M / GB,
             args.nconn, args.failure_domain))


def twemcache_format_output(config):
  return textwrap.dedent('''
    pelikan_twemcache config:
      hash_power:      {}
      slab_mem:        {} MB

    job config:
      cpu:             {}
      ram:             {} GB
      disk:            {} GB
      instances:       {}
      host limit:      {}
      rack limit:      {}

  '''.format(config['hash_power'], config['slab_mem'],
             config['cpu'], config['ram'], config['disk'],
             config['instance'], config['host_limit'], config['rack_limit']))


def slimcache_format_output(config):
  return textwrap.dedent('''
    pelikan_slimcache config:
      item_size:       {}
      nitem:           {}

    job config:
      cpu:             {}
      ram:             {} GB
      disk:            {} GB
      instances:       {}
      host limit:      {}
      rack limit:      {}

  '''.format(config['item_size'], config['nitem'],
             config['cpu'], config['ram'], config['disk'],
             config['instance'], config['host_limit'], config['rack_limit']))


# parser for calculator: to be included by the generator as a parent parser
parser = argparse.ArgumentParser(
  formatter_class=argparse.RawDescriptionHelpFormatter,
  description=textwrap.dedent("""
    This script calculates resource requirement of a pelikan cluster (twemcache or slimcache)
    based on input. It has to be run from the top level directory of source.\n

    Optional arguments that probably should be overwritten:
      qps, size, nkey, nconn

    Optional arguments:
      failure_domain (default to 5%, acceptable range {:.1f}% - {:.1f}%)
    """.format(FAILURE_DOMAIN_LOWER, FAILURE_DOMAIN_UPPER)),
  usage='%(prog)s [options]')

parser.add_argument('--qps', dest='qps', type=int, default=DEFAULT_QPS,
                    help='query per second in *thousands/K*, round up')
parser.add_argument('--size', dest='size', type=int, default=DEFAULT_SIZE,
                    help='key+value size in bytes, average for twemcache, max for slimcache')
parser.add_argument('--nkey', dest='nkey', type=int, default=DEFAULT_NKEY,
    help='number of keys in *millions/M*, round up')
parser.add_argument('--nconn', dest='nconn', type=int, default=DEFAULT_NCONN,
    help='number of connections to each server')
parser.add_argument('--failure_domain', dest='failure_domain', type=float,
    default=DEFAULT_FAILURE_DOMAIN,
    help='percentage of server/data that may be lost simultaneously')
parser.add_argument('--ram', nargs='+', type=int, default=RAM_CANDIDATES,
    help='provide a (sorted) list of container ram sizes to consider')
# end of parser

if __name__ == "__main__":
  # add runnable as a positional option instead of subparser (as in aurora.py) to avoid import
  parser.add_argument('runnable', choices=['twemcache', 'slimcache'], help='flavor of backend')
  format_output = {'twemcache': twemcache_format_output, 'slimcache': slimcache_format_output}
  args = parser.parse_args()
  print(format_input(args))
  config = calculate(args)
  print(format_output[args.runnable](config))
  print('Cluster sizing is primarily driven by {}.\n'.format(config['bottleneck']))

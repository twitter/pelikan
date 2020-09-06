import argparse
from math import ceil
import os
import sys
import subprocess
import re


PREFIX = 'loadgen'
PELIKAN_ITEM_OVERHEAD = {"twemcache": 48, "segcache": 8, "slimcache": 0}
PELIKAN_SERVER_PORT = 12300


def generate_config(rate, connections, ksize, vsize, mem_bytes, get_weight, set_weight, threads, backend):
# create rpcperf.toml
    item_overhead = PELIKAN_ITEM_OVERHEAD[backend]
    nkey = int(ceil(0.20 * mem_bytes / (ksize + vsize + item_overhead)))
    conn_per_thread = int(connections / threads)

    config_str = '''
[general]
clients = {threads}
tcp_nodelay = true
poolsize = {connections} # this specifies number of connection per thread
# runtime ~= windows x duration
windows = 3
interval = 60
request_ratelimit = {rate}
soft_timeout = true

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
        ksize=ksize, vsize=vsize, get_weight=get_weight, set_weight=set_weight)

    with open('rpcperf.toml', 'w') as the_file:
        the_file.write(config_str)

def get_hw_thd_idx():
    regex = re.compile(r"NUMA node(?P<numa_idx>\d+) CPU\(s\): *(?P<thd_start>\d+)-(?P<thd_end>\d+),\d+-\d+")
    thd_idx_pair = []
    p = subprocess.run("lscpu|grep NUMA", shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout = p.stdout.decode().split("\n")
    numa_n = 1
    for line in stdout:
        if "NUMA node(s)" in line:
            numa_n = int(line.split(":")[1])
        m = regex.search(line)
        if m:
            thd_idx_pair.append((int(m.group("thd_start")), int(m.group("thd_end"))))
    assert numa_n == len(thd_idx_pair), "{} {}".format(numa_n, thd_idx_pair)

    thd_idx = []
    for p in thd_idx_pair:
        for i in range(p[0], p[1]+1):
            thd_idx.append(i)
    return thd_idx


def generate_runscript(binary, server_ip, instances):
    # create test.sh
    fname = 'test.sh'
    with open(fname, 'w') as the_file:
        for i in range(instances):
            server_port = PELIKAN_SERVER_PORT + i
            the_file.write('ulimit -n 65536; ')
            the_file.write('taskset -c {core_id} {binary_file} --config {config_file}'.format(
                core_id=i % os.cpu_count(), binary_file=binary, config_file='rpcperf.toml'))
            the_file.write(' --endpoint {server_ip}:{server_port}'.format(
                server_ip=server_ip, server_port=server_port))
            the_file.write(' --waterfall latency-waterfall-{server_port}.png'.format(
                server_port=server_port))
            the_file.write(' > rpcperf_{server_port}_{instance_idx}.log'.format(
                server_port=server_port, instance_idx=i))
            the_file.write(' 2>&1 &\n')

        the_file.write("""
sleep 100;        
nrunning=1
while [ $nrunning -gt 0 ]
do
    nrunning=$(pgrep -c rpc-perf)
    echo "$(date) $(hostname): $nrunning clients are still running"
    sleep 10
done
""")
    os.chmod(fname, 0o777)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="""
        Generate all the client-side scripts/configs needed for a test run.
        """)
    parser.add_argument('--binary', dest='binary', type=str, help='location of rpc-perf binary', required=True)
    parser.add_argument('--backend', dest='backend', type=str, help='backend', required=True)
    parser.add_argument('--prefix', dest='prefix', type=str, default=PREFIX, help='folder that contains all the other files to be generated')
    parser.add_argument('--instances', dest='instances', type=int, help='number of instances', required=True)
    parser.add_argument('--server_ip', dest='server_ip', type=str, help='server ip', required=True)
    parser.add_argument('--rate', dest='rate', type=int, help='request rate per instance', required=True)
    parser.add_argument('--connections', dest='connections', type=int, help='number of connections per instance', required=True)
    parser.add_argument('--ksize', dest='ksize', type=int, help='key size', required=True)
    parser.add_argument('--vsize', dest='vsize', type=int, help='value size', required=True)
    parser.add_argument('--mem_bytes', dest='mem_bytes', type=int, help='memory size', required=True)
    parser.add_argument('--get_weight', dest='get_weight', type=int, help='get weight (0-10)', required=True)
    parser.add_argument('--set_weight', dest='set_weight', type=int, help='set weight (0-10)', required=True)
    parser.add_argument('--threads', dest='threads', type=int, help='number of worker threads per rpc-perf', required=True)

    args = parser.parse_args()

    if not os.path.exists(args.prefix):
        os.makedirs(args.prefix)
    os.chdir(args.prefix)

    generate_config(args.rate, args.connections,
                    args.ksize, args.vsize, args.mem_bytes,
                    args.get_weight, args.set_weight, args.threads, args.backend)
    generate_runscript(args.binary, args.server_ip, args.instances)

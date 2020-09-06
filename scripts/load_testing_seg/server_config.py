import argparse
from math import ceil, floor, log
import os
import subprocess
import sys

PREFIX = 'test'
PELIKAN_ADMIN_PORT = 9900
PELIKAN_SERVER_PORT = 12300
PELIKAN_ITEM_OVERHEAD = {"twemcache": 48, "segcache": 8, "slimcache": 0}
NUMA_NODE = 0

def generate_config(instances, ksize, vsize, mem_bytes, pmem_paths, engine, worker_binding):
    # create top-level folders under prefix
    try:
        os.makedirs('config')
    except:
        pass
    try:
        os.makedirs('log')
    except:
        pass

    item_overhead = PELIKAN_ITEM_OVERHEAD[engine]
    item_size = ksize + vsize + item_overhead
    # because segcache does not perform in-place update
    # we need to reduce the #keys
    nkey = int(ceil(0.20 * mem_bytes / item_size))
    hash_power = int(ceil(log(nkey, 2)))

    # create twemcache|slimcache|segcache config file(s)
    for i in range(instances):
        admin_port = PELIKAN_ADMIN_PORT + i
        server_port = PELIKAN_SERVER_PORT + i
        config_file = '{engine}-{server_port}.config'.format(engine=engine, server_port=server_port)

        # String with common options for both twemcache, segcache and slimcache
        config_str = """\
daemonize: yes
admin_port: {admin_port}
server_port: {server_port}

admin_tw_cap: 2000

buf_init_size: 4096

buf_sock_poolsize: 16384

debug_log_level: 4
debug_log_file: log/{engine}-{server_port}.log
debug_log_nbuf: 1048576

klog_file: log/{engine}-{server_port}.cmd
klog_backup: log/{engine}-{server_port}.cmd.old
klog_sample: 100
klog_max: 1073741824

prefill: yes
prefill_ksize: {ksize}
prefill_vsize: {vsize}
prefill_nkey: {nkey}

request_poolsize: 16384
response_poolsize: 32768

time_type: 2
""".format(admin_port=admin_port, server_port=server_port, ksize=ksize, vsize=vsize, nkey=nkey, engine=engine)

        # String with options specific for either twemcache or slimcache
        pmem_path_str = ""
        worker_pinning_str = ""
        if engine == "slimcache":
            datapool_param = "cuckoo_datapool"
            engine_str = """\

cuckoo_item_size: {item_size}
cuckoo_nitem: {nkey}
cuckoo_datapool_prefault: yes
""".format(item_size=item_size, nkey=nkey)
        elif engine == "twemcache":
            datapool_param = "slab_datapool"
            engine_str = """\

slab_evict_opt: 1
slab_prealloc: yes
slab_hash_power: {hash_power}
slab_mem: {mem_bytes}
slab_size: 1048576
slab_datapool_prefault: yes

stats_intvl: 10000
stats_log_file: log/twemcache-{server_port}.stats
""".format(hash_power=hash_power, mem_bytes=mem_bytes, server_port=server_port)
        elif engine == "segcache":
                datapool_param = "seg_datapool_path"
                engine_str = """\

seg_evict_opt: 1
seg_hash_power: {hash_power}
seg_mem: {mem_bytes}
seg_size: 1048576

stats_intvl: 10000
stats_log_file: log/segcache-{server_port}.stats
""".format(hash_power=hash_power, mem_bytes=mem_bytes, server_port=server_port)
        else:
                raise RuntimeError("unknown binary {}".format(engine))

        if worker_binding:
            worker_pinning_str = """

worker_binding_core: {worker_binding_core}
""".format(worker_binding_core=i)

        # String with option specific for PMEM usage
        if len(pmem_paths) > 0:
                pmem_path_str = """\

{datapool_param}: {pmem_path}
""".format(datapool_param=datapool_param, pmem_path=os.path.join(pmem_paths[i%len(pmem_paths)], 'pool_{}'.format(server_port)))

        # Put it all together
        config_str = config_str + engine_str + pmem_path_str + worker_pinning_str
        with open(os.path.join('config', config_file),'w') as the_file:
            the_file.write(config_str)

def generate_runscript(binary, instances, pmem_paths_count, engine, use_adq, worker_binding):
    # create bring-up.sh
    fname = 'bring-up.sh'
    numa_node_count = pmem_paths_count if pmem_paths_count > 0 else 1
    with open(fname, 'w') as the_file:
        the_file.write("ulimit -n 65536;\n")
        for i in range(instances):
            config_file = os.path.join('config', '{engine}-{server_port}.config'.format(engine=engine, server_port=PELIKAN_SERVER_PORT+i))
            if not worker_binding:
                the_file.write('sudo numactl --cpunodebind={numa_node} --preferred={numa_node} '.format(
                        numa_node=i%numa_node_count))

            if use_adq:
                the_file.write("sudo cgexec -g net_prio:{cgroup_name} --sticky ".format(cgroup_name="app_tc1"))

            the_file.write('sudo {binary_file} {config_file} > server.log 2>&1 \n'.format(
                    binary_file=binary, config_file=config_file))
    os.chmod(fname, 0o777)

    # create warm-up.sh
    fname = 'warm-up.sh'
    prefill_opt = ""
    if engine == "slimcache":
        prefill_opt = "prefilling cuckoo"
    elif engine == "twemcache":
        prefill_opt = "prefilling slab"
    elif engine == "segcache":
        prefill_opt = "prefilling seg"

    with open(fname, 'w') as the_file:
        the_file.write("""
./bring-up.sh

nready=0
while [ $nready -lt {instances} ]
do
        nready=$(grep -l "{prefill_opt}" log/{engine}-*.log | wc -l)
        echo "$(date): $nready out of {instances} servers are warmed up"
        sleep 10
done
""".format(instances=instances, prefill_opt=prefill_opt, engine=engine))
    os.chmod(fname, 0o777)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="""
        Generate all the server-side scripts/configs needed for a test run.
        """)
    parser.add_argument('--binary', dest='binary', type=str, help='location of pelikan_twemcache|pelikan_slimcache binary', required=True)
    parser.add_argument('--prefix', dest='prefix', type=str, default=PREFIX, help='folder that contains all the other files to be generated')
    parser.add_argument('--instances', dest='instances', type=int, help='number of instances')
    parser.add_argument('--ksize', dest='ksize', type=int, help='key size')
    parser.add_argument('--vsize', dest='vsize', type=int, help='value size')
    parser.add_argument('--mem_bytes', dest='mem_bytes', type=int, help='total capacity of heap memory, in bytes')
    parser.add_argument('--use_adq', dest='use_adq', default=False, type=str, help='whether to use adq')
    parser.add_argument('--worker_binding', dest='worker_binding', default=True, type=bool, help='whether binding worker thread to cores')
    parser.add_argument('--pmem_paths', dest='pmem_paths', nargs='*', help='list of pmem mount points')

    args = parser.parse_args()

    if not os.path.exists(args.prefix):
        os.makedirs(args.prefix)
    os.chdir(args.prefix)
    print(os.getcwd(), args.prefix)

    engine = ""
    binary_help_out = subprocess.run([args.binary, '--help'], stdout=subprocess.PIPE).stdout.decode()
    for e in ("twemcache", "segcache", "slimcache"):
        if e in binary_help_out:
            engine = e
            break
    if len(engine) == 0:
        print('Provided binary is not twemcache|segcache|slimcache. Only these engines are valid. Exiting...')
        print("binary help output: {}".format(binary_help_out))
        sys.exit()

    use_adq = True if args.use_adq == "1" or args.use_adq == "True" else False
    generate_config(args.instances, args.ksize, args.vsize, args.mem_bytes, args.pmem_paths, engine, args.worker_binding)
    generate_runscript(args.binary, args.instances, len(args.pmem_paths), engine, use_adq, args.worker_binding)

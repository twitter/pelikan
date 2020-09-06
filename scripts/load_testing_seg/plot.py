import os, sys
import argparse
import re
import glob
from collections import namedtuple
from pprint import pprint
import numpy as np
import matplotlib
import matplotlib.pyplot as plt

sys.path.append(os.path.expanduser("~/"))
sys.path.append(os.path.expanduser("~/myworkspace/"))

try:
    import JPlot
    from JPlot import pyplot as plt
    from JPlot import plotTools as plotTools

    JPlot.set_auto_open(False)
    JPlot.set_plot_style("presentation-onecol")
    from matplotlib.ticker import MaxNLocator
except Exception as e:
    print(e)


Config = namedtuple("Config", ["thrpt", "nconn", "item_sz"])
LatencyRes = namedtuple('LatencyRes',
                        ["thrpt", "conn", "P25", "P50", "P75", "P90", "P99", "P999", "P9999"])

n_instance = 24
n_host = 28
FACTOR = n_instance * n_host


def parse_data(data_path):
    result_started = False
    regex_conn = re.compile(r"Connections: Attempts: (?P<attempt>\d+) Opened: (?P<opened>\d+) Errors: (?P<error>\d+) Timeouts: (?P<timeout>\d+) Open: (?P<open>\d+)")
    regex_rps = re.compile(r"Rate: Request: (?P<req>[0-9.]+) rps Response: (?P<resp>[0-9.]+) rps Connect: (?P<conn>[0-9.]+) cps")
    regex_lat = re.compile(r"Request Latency \(us\): p25: (?P<P25>\d+) p50: (?P<P50>\d+) p75: (?P<P75>\d+) p90: (?P<P90>\d+) p99: (?P<P99>\d+) p999: (?P<P999>\d+) p9999: (?P<P9999>\d+)")

    thrpt, conn, P25, P50, P75, P90, P99, P999, P9999 = 0, 0, 0, 0, 0, 0, 0, 0, 0
    with open(data_path) as ifile:
        for line in ifile:
            if not result_started and "Window: 2" not in line:
                continue
            if "Window: 2" in line:
                result_started = True
                continue
            if "Connections" in line:
                m = regex_conn.search(line)
                conn = int(m["open"])
                continue
            if "Rate:" in line:
                m = regex_rps.search(line)
                thrpt = float(m["req"]) / 1000000
                continue
            if "Request Latency " in line:
                m = regex_lat.search(line)
                if m is None:
                    print("{}\n{}".format(data_path, line))
                else:
                    P25, P50, P75, P90, P99, P999, P9999 = \
                        int(m["P25"]), int(m["P50"]), int(m["P75"]), int(m["P90"]),\
                        int(m["P99"]), int(m["P999"]), int(m["P9999"])
                continue

    return LatencyRes(thrpt=thrpt * n_instance * n_host, conn=conn * n_host,
                      P25=P25, P50=P50, P75=P75, P90=P90, P99=P99, P999=P999, P9999=P9999)

def print_data(data_path):
    print("{:8} {:8} {:8} {:8} {:8} {:8} {:8} {:8} {:8} {:8}".format(
        "P25", "P50", "P75", "P90", "P99", "P999", "P9999", "MQPS",
        "connection", "item size"
    ))
    for thrpt in (0.5, 2):
        for item_size in (64, 4096):
            for nconn in (100, 500, 1000, 2000, 5000, 10000, ):
                configs = []
                for f in glob.glob("{}/rpcperf_{}_{}_{}*".format(data_path, thrpt, nconn, item_size)):
                    configs.append(parse_data(f))
                    # print(configs[-1], f.split("/")[-1])
                if len(configs) == 0:
                    print("{:8} {:8} {:8} {:8} {:8} {:8} {:8} {:8.4} {:8} {:8}".format(
                        "noData", "noData", "noData", "noData", "noData", "noData",
                        "noData", "noData", "noData", "noData",
                    ))
                else:
                    print("{:8} {:8} {:8} {:8} {:8} {:8} {:8} {:8.4} {:8} {:8}".format(
                        np.mean(sorted([config.P25 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.P50 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.P75 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.P90 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.P99 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.P999 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.P9999 for config in configs])[2:-2]).astype(int),
                        np.mean(sorted([config.thrpt for config in configs])[2:-2]), nconn, item_size
                    ))
            print()


def get_data(data_path, host):
    data_dict = {}
    for f in os.listdir(data_path):
        # rpcperf_16_100_1024_12000_0.log.smf1-iio-07-sr1
        _, thrpt, nconn, item_sz, instance_idx, other = f.split("_")
        _, _, curr_host = other.split(".")
        if int(float(thrpt)) == 16 or curr_host != host:
            continue
        conf = Config(thrpt=float(thrpt), nconn=int(nconn), item_sz=int(item_sz))
        lat = parse_data("{}/{}".format(data_path, f))
        data_dict[conf] = lat

    pprint(data_dict)
    return data_dict

def plot_lat(data_path_no_adq, data_path_adq):
    host = "smf1-ifm-23-sr1"
    # host = "smf1-ifp-26-sr1"
    # host = "smf1-hii-17-sr1"
    data_dict_no_adq = get_data(data_path_no_adq, host)
    data_dict_adq = get_data(data_path_adq, host)
    conn_counts = (100, 500, 1000, 2000, 5000, 10000, 25000)
    x_ticks1 = [2*i+1.2 for i in range(len(conn_counts))]
    x_ticks2 = [2*i+1.8 for i in range(len(conn_counts))]

    for thrpt in (0.5, 1, 2, ):
        for item_sz in (64, 4096):
            P50_na, P90_na, P99_na, P999_na, P9999_na = [], [], [], [], []
            P50_ad, P90_ad, P99_ad, P999_ad, P9999_ad = [], [], [], [], []
            for nconn in conn_counts:
                conf = Config(float(thrpt), nconn=nconn, item_sz=item_sz)
                if conf not in data_dict_no_adq:
                    P50_na.append(0)
                    P90_na.append(0)
                    P99_na.append(0)
                    P999_na.append(0)
                else:
                    lat = data_dict_no_adq[conf]
                    P50_na.append(lat.P50)
                    P90_na.append(lat.P90)
                    P99_na.append(lat.P99)
                    P999_na.append(lat.P999)
                    P9999_na.append(lat.P9999)

                if conf not in data_dict_adq:
                    P50_ad.append(0)
                    P90_ad.append(0)
                    P99_ad.append(0)
                    P999_ad.append(0)
                else:
                    lat = data_dict_adq[conf]
                    P50_ad.append(lat.P50)
                    P90_ad.append(lat.P90)
                    P99_ad.append(lat.P99)
                    P999_ad.append(lat.P999)
                    P9999_ad.append(lat.P9999)

            P50_na, P90_na, P99_na, P999_na, P9999_na = np.array(P50_na), np.array(P90_na), np.array(P99_na), np.array(P999_na), np.array(P9999_na)
            P50_ad, P90_ad, P99_ad, P999_ad, P9999_ad = np.array(P50_ad), np.array(P90_ad), np.array(P99_ad), np.array(P999_ad), np.array(P9999_ad)

            plt.bar(x_ticks1, P50_na,  width=0.48, hatch="/",  color="red", alpha=0.64,    edgecolor='white')
            plt.bar(x_ticks1, P90_na,  width=0.48, hatch="\\", color="green", alpha=0.64,  bottom=P50_na, edgecolor='white')
            plt.bar(x_ticks1, P99_na,  width=0.48, hatch="*",  color="blue", alpha=0.64,   bottom=P50_na+P90_na, edgecolor='white')
            plt.bar(x_ticks1, P999_na, width=0.48, hatch="o", color="grey", alpha=0.64, bottom=P50_na+P90_na+P99_na, edgecolor='white')

            plt.bar(x_ticks2, P50_ad,  width=0.48, hatch="/",  color="red", alpha=0.64,    edgecolor='white')
            plt.bar(x_ticks2, P90_ad,  width=0.48, hatch="\\", color="green", alpha=0.64,  bottom=P50_ad, edgecolor='white')
            plt.bar(x_ticks2, P99_ad,  width=0.48, hatch="*",  color="blue", alpha=0.64,   bottom=P50_ad+P90_ad, edgecolor='white')
            plt.bar(x_ticks2, P999_ad, width=0.48, hatch="o", color="grey", alpha=0.64, bottom=P50_ad+P90_ad+P99_ad, edgecolor='white')

            max_y = max(np.max(P999_na), np.max(P999_ad))
            if max_y > 1e5:
                yticks = (100, 1000, 5000, 10000, 100000)
            elif max_y > 1e4:
                yticks = (100, 1000, 2000, 5000, 10000)
            elif max_y > 1e3:
                yticks = (100, 500, 1000, 2000, 5000)
            else:
                yticks = (100, 200, 500, 1000, 2000)
            plt.legend(["P50", "P90", "P99", "P999"], ncol=4)
            plt.yscale("log")
            plt.xticks(x_ticks1, conn_counts)
            plt.yticks(yticks, yticks)
            plt.grid(linestyle="--")
            plt.ylabel("latency ($\mu$s)")
            plt.xlabel("Connection count")
            plt.title("Throughput {:.1f} M QPS, item size {}".format(thrpt, item_sz))
            plt.savefig("lat_{}_{}.png".format(thrpt, item_sz))
            plt.clf()

def used():
    plot_lat("rpcperf_log_mellanox_40Gbps_smf1-dwk-27-sr1_tuning", "rpcperf_log_mellanox_adq3", )
    plot_lat("rpcperf_log_mellanox_adq3", "rpcperf_log_no_adq1")
    plot_lat("rpcperf_log_no_adq_newfirmware", "rpcperf_log_adq_newfirmware")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="""
        parse rpcperf results and print/plot the results
        """)
    parser.add_argument('--mode', dest='mode', type=str, help='print or plot', default="print", )
    parser.add_argument('--data', dest='data', type=str, help='path to data folder', required=True)
    parser.add_argument('--data2', dest='data2', type=str, help='path to data2 folder, required to plotting', required=False)

    args = parser.parse_args()

    if args.mode == "print":
        print_data(args.data)
    elif args.mode == "plot":
        plot_lat(args.data, args.data2)
    else:
        parser.print_help()
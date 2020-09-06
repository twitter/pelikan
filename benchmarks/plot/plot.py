

import os, sys, time
from collections import defaultdict
from pprint import pprint
import matplotlib
import matplotlib.pyplot as plt

KiB = 1024
MiB = 1024 * KiB
GiB = 1024 * MiB

def parse_output(output_file):
    d = defaultdict(list)

    with open(output_file, "r") as ifile:
        for line in ifile:
            if "conf" in line:
                conf_name = line.split()[4]
                dev, cache, var, v = conf_name.split(".")[0].split("_")

            if line.startswith("Latency"):
                line_split = line.split()
                op = line_split[5].replace("delete", "del")
                p50 = int(float(line_split[8].strip(",")))
                p99 = int(float(line_split[9].strip(",")))
                p999 = int(float(line_split[10].strip(",")))
                d["{}_{}_{}_{}_p50".format(dev, cache, var, op)].append(p50)
                d["{}_{}_{}_{}_p99".format(dev, cache, var, op)].append(p99)
                # d["{}_{}_{}_{}_p999".format(dev, cache, var, op)].append(p999)

    for k, v in d.items():
        print("{}: {}".format(k, v))

    return d

def plot_dram_pmem(data_file):
    d = parse_output(data_file)
    for pos in range(0, 5):
        for p in ("p50", ):
            dram = [d["dram_slab_n_get_{}".format(p)][pos],
                    d["dram_slab_n_set_{}".format(p) ][pos],
                    d["dram_slab_n_del_{}".format(p) ][pos],
                    d["dram_seg_n_get_{}".format(p) ][pos],
                    d["dram_seg_n_set_{}".format(p) ][pos],
                    d["dram_seg_n_del_{}".format(p) ][pos], ]

            pmem = [d["pmem_slab_n_get_{}".format(p) ][pos],
                    d["pmem_slab_n_set_{}".format(p) ][pos],
                    d["pmem_slab_n_del_{}".format(p) ][pos],
                    d["pmem_seg_n_get_{}".format(p) ][pos],
                    d["pmem_seg_n_set_{}".format(p) ][pos],
                    d["pmem_seg_n_del_{}".format(p) ][pos], ]

            fig, ax = plt.subplots()
            ax.bar([0,2,4,6,8,10], dram, width=0.4, hatch="+",
                    label="dram")
            s = 0.4
            ax.bar([1-s, 3-s, 5-s, 7-s, 9-s, 11-s], pmem, width=0.4, hatch=".",
                    label="pmem",
                    tick_label=("slab_get", "slab_set", "slab_del",
                                "seg_get", "seg_set", "seg_del")
                    )
            # plt.xlabel()
            _ = ax.set(xticks=[1-2*s, 3-2*s, 5-2*s, 7-2*s, 9-2*s, 11-2*s], xticklabels=
                        ("slab_get", "slab_set", "slab_del",
                         "seg_get", "seg_set", "seg_del"))
            plt.title("working set {} GiB".format(64 * (1 << (20+2*pos)) // GiB))
            plt.ylabel("Latency (ns)")
            plt.legend()
            plt.xticks(rotation=90)
            plt.tight_layout()
            plt.savefig("dram_pmem_{}.png".format(pos))
            plt.clf()


def entry_size(data_file):
    default_entry_size, default_n_entry = 64, 65536
    entry_size = list(["{}".format(1<<i) for i in range(6, 17, 2)])
    # print(entry_size)
    n_entry = list([1<<i for i in range(16, 25)])
    n_point = 6

    d = parse_output(data_file)

    for op in ("get", "set", "del"):
        for p in ("p50", "p99"):
            plt.plot(entry_size[:n_point],
                     d["dram_slab_sz_{}_{}".format(op, p) ][:n_point],
                     marker="o", label="dram_slab")
            plt.plot(entry_size[:n_point],
                     d["dram_seg_sz_{}_{}".format(op, p) ][:n_point],
                     marker="o", label="dram_seg")
            plt.plot(entry_size[:n_point],
                     d["pmem_slab_sz_{}_{}".format(op, p) ][:n_point],
                     marker="o", label="pmem_slab")
            plt.plot(entry_size[:n_point],
                     d["pmem_seg_sz_{}_{}".format(op, p) ][:n_point],
                     marker="o", label="pmem_seg")
            plt.xlabel("entry size (bytes)")
            plt.ylabel("latency (ns)")
            plt.yticks((100, 1000, 10000))
            plt.yscale("log")
            plt.title("{} {}".format(op, p))
            plt.legend()
            plt.grid(linestyle="--")
            plt.tight_layout()
            plt.savefig("sz_{}_{}.png".format(op, p))
            plt.clf()



if __name__ == "__main__":
    # parse_output("output2")
    # plot_dram_pmem("output_n")
    # entry_size("output")

    plot_dram_pmem("output_new")
    entry_size("output_new")



import os, sys
import struct
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from collections import defaultdict

def data():
    return {
        # "gizmoduck": {432000: 1},
        # "tweetypie": {86400: 1},
        # "strato_negative_result": {2592000: 1},
        # "livepipeline": {120:0.96, 2700:0.04},
        #
        # "media_metadata": {1209600:0.75, 60:0.25},
        # "simclusters_v2_entity_cluster_scores": {28800: 1},
        # "graph_feature_service": {25920: 1},
        # "passbird": {10800:0.56, 103680: 0.44},
        # "content_recommender": {500: 1},
        # "blender_adaptive": {60:0.39, 300:0.24, 3600:0.13, 600:0.12, 14400:0.12},
        # "talon": {1296000: 1},
        # "pinkfloyd": {3600: 0.67, 7200: 0.33},
        #
        # "gizmoduck_lru": {43200: 1},
        # "expandodo": {86400:1.00},
        # "safety_label_store": {2592000:1.00},
        # "ranking_user_data_fetch": {60:1.00},
        # "exchange_auctioncache": {300:0.98, 299:0.02},
        # "simclusters_core_esc":	{28800:0.95, 86400:0.05},
        # "taxi_v3_prod":	{43200:0.58, 3600:0.42},
        # "wtf_req": {13680.0: 0.13, 11880.0: 0.13, 12240.0: 0.13, 14040.0: 0.12, 12600.0: 0.12, 12960.0: 0.12, 13320.0: 0.11, 14400: 0.07, 11520.0: 0.07},

        "timelines_content_features": {2*86400:1.00},
        "pushservice_mh": {10*3600:0.4, 11*86400:0.4, 9*86400:0.1, 12*86400:0.1},
        "devices": {5*86400:0.97, 20:0.03},

        # "mix1": {2592000: 0.2, 43200: 0.2, 86400: 0.2, 7200: 0.2, 240: 0.2},
        # "mix1": {2592000: 0.14, 43200: 0.38, 86400:0.23, 1209600:0.11, 120:0.12, 2700:0.02},
        # "mix2": {432000: 0.2, 1296000: 0.2, 64000: 0.2, 60: 0.2, 900: 0.2},
        # "mix3": {43200: 0.2, 129600: 0.2, 60: 0.2, 2592000: 0.2, 259200: 0.2},

        # "prediction_from_ranking": {60: 1},
        # "ads_merged_counting_features": {240: 1},
    }


def cal_compulsory_miss_ratio(data_file, default_ttl_dict):
    s = struct.Struct("<IQII")
    last_write_ts_dict = {}
    ttl_dict = {}
    n_miss = 0
    n_read_req = 0
    n_req = 0
    total_working_set = 0
    default_ttl_list = []
    ttl_list_idx = 0
    for ttl, perc in default_ttl_dict.items():
        for _ in range(int(perc * 100+0.4999)):
            default_ttl_list.append(ttl)
    assert len(default_ttl_list) == 100, len(default_ttl_list)

    with open(data_file, "rb") as ifile:
        r = ifile.read(s.size)
        while r:
            ts, obj_id, kv_len, op_ttl = s.unpack(r)
            n_req += 1
            if n_req % 100000000 == 0:
                t = time.localtime(time.time())
                # print("{} {}".format(time.strftime("%H:%M:%S"), n_req))
            r = ifile.read(s.size)
            klen = (kv_len >> 22) & (0x00000400 - 1)
            vlen = kv_len & (0x00400000 - 1)
            if klen + vlen > 1048540:
                print("{} has large item with size {}".format(data_file, klen+vlen))

            op = (op_ttl >> 24) & (0x00000100 - 1)
            ttl = op_ttl & (0x01000000 - 1)

            if 2 < op < 7:
                if (ttl == 0):
                    print("ts {} write {} klen {} vlen {} with ttl {}".format(ts, obj_id, klen, vlen, ttl))
                    ttl_dict[obj_id] = default_ttl_list[ttl_list_idx]
                    ttl_list_idx = (ttl_list_idx + 1) % 100
                last_write_ts_dict[obj_id] = ts
                ttl_dict[obj_id] = ttl
                if obj_id not in last_write_ts_dict:
                    total_working_set += klen + vlen
                continue

            n_read_req += 1

            if obj_id in last_write_ts_dict:
                last_ts = last_write_ts_dict.get(obj_id)
                if ts > last_ts + ttl_dict[obj_id]:
                    # expired
                    n_miss += 1
                    last_write_ts_dict[obj_id] = ts
                    ttl_dict[obj_id] = default_ttl_list[ttl_list_idx]
                    ttl_list_idx = (ttl_list_idx + 1) % 100

            else:
                n_miss += 1
                total_working_set += klen + vlen
                last_write_ts_dict[obj_id] = ts
                ttl_dict[obj_id] = default_ttl_list[ttl_list_idx]
                ttl_list_idx = (ttl_list_idx + 1) % 100

    print("{} miss ratio {:.4f}, workingset {:.4} GiB, {} objects, {} read_req, "
          "{} req".format(
        data_file.split("/")[-1],
        n_miss/n_read_req, total_working_set/1024/1024/1024,
        len(last_write_ts_dict), n_read_req, n_req,
    ))

def cal_overwrite_ratio(data_file):
    s = struct.Struct("<IQII")
    last_write_ts = {}
    n_overwrite, n_write = 0, 0
    n_req = 0
    with open(data_file, "rb") as ifile:
        r = ifile.read(s.size)
        while r:
            ts, obj_id, kv_len, op_ttl = s.unpack(r)
            n_req += 1
            if n_req == 100000000:
                break
            r = ifile.read(s.size)
            op = (op_ttl >> 24) & (0x00000100 - 1)

            if 2 < op < 7:
                if obj_id in last_write_ts:
                    n_overwrite += 1
                last_write_ts[obj_id] = ts
                n_write += 1
    print("{} {}/{} overwrite ratio {:.4f}".format(data_file, n_overwrite, n_write, n_overwrite/n_write))

def per_obj_overwrite_ratio(data_file):
    s = struct.Struct("<IQII")
    last_write_ts = {}
    n_overwrite = defaultdict(int)
    n_req = 0
    with open(data_file, "rb") as ifile:
        r = ifile.read(s.size)
        while r:
            ts, obj_id, kv_len, op_ttl = s.unpack(r)
            n_req += 1
            if n_req == 100000000:
                break
            r = ifile.read(s.size)
            op = (op_ttl >> 24) & (0x00000100 - 1)

            if 2 < op < 7:
                if obj_id in last_write_ts:
                    n_overwrite[obj_id] += 1
                last_write_ts[obj_id] = ts
    l = sorted(n_overwrite.values(), reverse=True)
    n_overwrite_total = sum(n_overwrite.values())
    l2 = ["{:.4f}".format(n/n_overwrite_total) for n in l[:10]]
    print("{} {}".format(data_file, l2))


def run():
    BASE_DATA_DIR = "/data/twoDay/"
    # cal_compulsory_miss_ratio("/Users/junchengy/tweetypie.bin.processed", {86400: 1})
    # cal_compulsory_miss_ratio("/data/twoDay/mix1_cache.sbin", data()["mix1"])
    # for cache, default_ttl_dict in data().items():
    #     data_path = "{}/{}_cache.sbin".format(BASE_DATA_DIR, cache)
        # cal_compulsory_miss_ratio(data_path, default_ttl_dict)
        # cal_overwrite_ratio(data_path)
        # per_obj_overwrite_ratio(data_path)


    futures_dict = {}
    with ProcessPoolExecutor() as ppe:
        for cache, default_ttl_dict in data().items():
            futures_dict[ppe.submit(cal_compulsory_miss_ratio, "{}/{}_cache.sbin".format(BASE_DATA_DIR, cache), default_ttl_dict)] = cache
            # futures_dict[ppe.submit(cal_overwrite_ratio, "{}/{}_cache.sbin".format(BASE_DATA_DIR, cache))] = cache
            # futures_dict[ppe.submit(per_obj_overwrite_ratio, "{}/{}_cache.sbin".format(BASE_DATA_DIR, cache))] = cache
        for future in as_completed(futures_dict):
            result = future.result()


if __name__ == "__main__":
    run()



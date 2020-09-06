

import os
import struct
from multiprocessing import Process 


DATA_PATH = "/data/twoDay/"
mix1 = ("strato_negative_result", "gizmoduck_lru", "tweetypie", "livepipeline", )
mix2 = ("ibis_api", "talon", "limiter_feature_med", "ranking_user_data_fetch", "timelines_ranked_tweet", )
mix3 = ("sc_knownfors", "media_lshstore", "escherbird_utt_prod", "real_time_tfe_feeder_production", "twemcache_escherbird_top_terms_and_retweets", )



def mix_workload(data_list, ofile_path, add_label=False):
    s = struct.Struct("<IQII")
    file_list = []
    curr_req_list = []
    for data in data_list:
        data_path = "{}/{}_cache.sbin".format(DATA_PATH, data)
        file_list.append(open(data_path, "rb"))
        curr_req_list.append(s.unpack(file_list[-1].read(s.size)))

    start_ts = [req[0] for req in curr_req_list]
    ofile = open(ofile_path, "wb")

    while len(file_list) > 0:
        min_idx, min_ts = 0, 1e20
        for idx, req in enumerate(curr_req_list):
            ts = req[0] - start_ts[idx]
            if ts < min_ts:
                min_idx = idx
                min_ts = ts

        kv_len = curr_req_list[min_idx][2]
        klen = (kv_len >> 22) & (0x00000400 - 1)
        vlen = kv_len & (0x00400000 - 1)
        assert vlen < 1048600, "{} {}".format(min_idx, data_list[min_idx])
        t = (curr_req_list[min_idx][0]-start_ts[min_idx], curr_req_list[min_idx][1], curr_req_list[min_idx][2], curr_req_list[min_idx][3])
        ofile.write(s.pack(*t))
        r = file_list[min_idx].read(s.size)
        if r:
            curr_req_list[min_idx] = s.unpack(r)
        else:
            curr_req_list.remove(curr_req_list[min_idx])
            file_list[min_idx].close()
            file_list.remove(file_list[min_idx])
    ofile.close()

def run():
    mix_workload(mix1, "{}/{}".format(DATA_PATH, "mix1_cache2.sbin"))
    # mix_workload(mix2, "{}/{}".format(DATA_PATH, "mix2_cache2.sbin"))
    # mix_workload(mix3, "{}/{}".format(DATA_PATH, "mix3_cache2.sbin"))
    # mix_workload(mix3, "{}".format("mix3.sbin"))

if __name__ == "__main__":
    run()



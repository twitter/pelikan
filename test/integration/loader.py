#!/usr/bin/env python

from __future__ import print_function
import re
import sys

re_empty = re.compile("^$")
re_req = re.compile("^>>> (.+)$")
re_rsp = re.compile("^<<< (.+)$")
re_stat = re.compile("^\+\+\+ (.+)$")


def split_metrics(line):
    """split a line of metric deltas into a dict, for example:

    'get +1, get_hit +1' -> {'get':1, 'get_hit':1}
    'request_free -1, request_parse +1' -> {'request_free':-1, 'request_parse':1}
    """
    metrics = line.split(',')
    d = {}
    for m in metrics:
        name,delta = m.strip().split(' ')
        d[name.strip()] = int(delta)

    return d


def load_seq(fname):
    """Load the test (a sequence of commands and asserts) from a file.

    Each command contains one or more lines of request, leading with '>>> ', and
    one or more lines of response, leading with '<<< '. Commands are separated
    by an empty line.
    """
    lines = open(fname).readlines()
    if not re_empty.match(lines[-1]):  # ensure an empty line at the end
      lines.append('\n')

    seq = []
    req = []
    rsp = []
    stat = {}
    for line in lines:
        if (re_empty.match(line)):  # reset
            if len(req) > 0:  # rsp and stat can be empty
                seq.append({'req': req, 'rsp':rsp, 'stat':stat})
            req = []
            rsp = []
            stat = {}
        elif re_req.match(line):
            req.append(re_req.match(line).group(1))
        elif re_rsp.match(line):
            rsp.append(re_rsp.match(line).group(1))
        elif re_stat.match(line):
            stat.update(split_metrics(re_stat.match(line).group(1)))
        else:
            print("unrecognized line")

    return seq

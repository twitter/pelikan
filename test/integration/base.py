from client import DataClient, AdminClient
from loader import load_seq
from server import PelikanServer

import unittest

DEFAULT_SERVER = ('localhost', 12321)
DEFAULT_ADMIN = ('localhost', 9999)


class GenericTest(unittest.TestCase):
    def setUp(self):
        self.server = PelikanServer('pelikan_twemcache')
        self.server.ready()
        self.data_client = DataClient(DEFAULT_SERVER)
        self.admin_client = AdminClient(DEFAULT_ADMIN)
        self.stats = self.admin_client.stats()

    def tearDown(self):
        self.data_client.close()
        self.admin_client.close()
        self.server.stop()

    def load(self, fname):
        """loading a test sequence from a file"""
        self.seq = load_seq(fname)

    def assertResponse(self, expected):
        """receive and verify response (a list) matches expectation"""
        rsp = self.data_client.response()
        self.assertEqual(len(rsp), len(expected))
        for i in range(len(rsp)):
            self.assertEqual(rsp[i], expected[i])

    def assertStats(self, delta):
        """delta, a dict, captures the expected change in a subset of metrics"""
        stats = self.admin_client.stats()
        for k in delta:
            self.assertEqual(stats[k] - self.stats[k], delta[k])
        self.stats = stats

    def runTest(self):
        for d in self.seq:
            self.data_client.request(d['req'])
            self.assertResponse(d['rsp'])
            self.assertStats(d['stat'])

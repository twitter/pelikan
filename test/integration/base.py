import unittest

from .pelikan_client import PelikanClient
from .pelikan_server import PelikanServer

SERVER_PORT = 12345
ADMIN_PORT = 12346


class PelikanTest(unittest.TestCase):
    reserved_ports = set()

    def setUp(self):
        self.server = self.getServer(
            server_port=SERVER_PORT,
            admin_port=ADMIN_PORT
        )
        self.client = PelikanClient(
            server_port=SERVER_PORT,
            admin_port=ADMIN_PORT
        )

    def tearDown(self):
        self.server.stop()

    def assertRead(self, expected):
        read = self.client.read(len(expected))
        self.assertEqual(expected, read)


    def assertMetrics(self, *args):
        stats = self.client.getStats()
        for (k, v) in args:
            self.assertEqual(
                stats.get(k, None),
                str(v),
                'Expected {} to be {}, got {} instead'.format(
                    k,
                    v,
                    stats.get(k, None),
                )
            )


class TwemcacheTest(PelikanTest):
    def getServer(self, **kwargs):
        server = PelikanServer('pelikan_twemcache')
        server.start(**kwargs)
        server.wait_ready()
        return server

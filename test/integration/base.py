import errno
import socket
import unittest

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
        self.client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.client.connect(('localhost', SERVER_PORT))
        self.admin = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.admin.connect(('localhost', ADMIN_PORT))

    def tearDown(self):
        self.server.stop()

    def assertRead(self, expected):
        read = self.client.recv(len(expected))
        self.assertEqual(expected, read)

    def getStats(self):
        self.admin.sendall('stats\r\n')
        data = ''
        self.admin.setblocking(False)
        while True:
            try:
                data += self.admin.recv(1024)
            except socket.error, e:
                err = e.args[0]
                if err == errno.EAGAIN or err == errno.EWOULDBLOCK:
                    if len(data) == 0:
                        continue
                    else:
                        break
                else:
                    raise
        self.admin.setblocking(True)
        # this NULL is kinda unexpected for me
        self.assertEqual(data[-6:], 'END\r\n\0')
        return dict(line.split(' ')[1:] for line in data[:-6].strip().split('\r\n'))

    def assertMetrics(self, *args):
        stats = self.getStats()
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

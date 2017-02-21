import contextlib
import errno
import os
import socket
import subprocess
import tempfile
import unittest


class PelikanTest(unittest.TestCase):
    reserved_ports = set()

    @staticmethod
    def default_path():
        return os.path.realpath(os.path.join(
            os.path.dirname(os.path.abspath(__file__)),
            '../../_build/_bin'
        ))

    # from http://stackoverflow.com/a/22106569
    @staticmethod
    def get_open_port(lowest_port = 10000, highest_port = None, bind_address = '', *socket_args, **socket_kwargs):
        if highest_port is None:
            highest_port = lowest_port + 100
        while lowest_port < highest_port:
            if lowest_port not in PelikanTest.reserved_ports:
                try:
                    with contextlib.closing(socket.socket(*socket_args, **socket_kwargs)) as my_socket:
                        my_socket.bind((bind_address, lowest_port))
                        this_port = my_socket.getsockname()[1]
                        PelikanTest.reserved_ports.add(this_port)
                        return this_port
                except socket.error as error:
                    if not error.errno == errno.EADDRINUSE:
                        raise
                    assert not lowest_port == 0
                    PelikanTest.reserved_ports.add(lowest_port)
            lowest_port += 1
        raise Exception('Could not find open port')

    def setUp(self):
        self.port = self.get_open_port()
        self.admin_port = self.get_open_port()

        self.config_file = tempfile.NamedTemporaryFile()
        self.config_file.write('server_port: {}\nadmin_port: {}\n'.format(
            self.port,
            self.admin_port,
        ))
        self.config_file.flush()

        executable = self.get_executable(os.getenv('PELIKAN_BIN_PATH', PelikanTest.default_path()))
        self.server = subprocess.Popen((executable, self.config_file.name),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # wait for the port to be listening; no great "ready" output present
        # but it lists all configs, so this must exist
        while 'name: server_port' not in self.server.stdout.readline():
            pass

        self.client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.client.connect(('127.0.0.1', self.port))
        self.admin = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.admin.connect(('127.0.0.1', self.admin_port))

    def tearDown(self):
        self.server.kill()

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
    def get_executable(self, bin_path):
        return os.path.join(bin_path, 'pelikan_twemcache')

import errno
import socket
import unittest

# implementation based on pymemcache: https://pypi.python.org/pypi/pymemcache

class TCPClient(object):
    DEFAULT_TIMEOUT = 1.0
    RECV_SIZE = 1024 * 1024

    def __init__(self,
                 server,
                 connect_timeout=DEFAULT_TIMEOUT,
                 request_timeout=DEFAULT_TIMEOUT,
                 recv_size=RECV_SIZE):
        """Constructor: setting and connecting to the remote."""
        self.server = server
        self.connect_timeout = connect_timeout
        self.request_timeout = request_timeout
        self.recv_size = recv_size
        self.connect()

    def connect(self):
        """Connect to a TCP host:port endpoint, return socket"""
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(self.connect_timeout)
        sock.connect(self.server)
        sock.settimeout(self.request_timeout)
        self.sock = sock

    def close(self):
        """Close the socket"""
        if self.sock is not None:
            try:
                self.sock.close()
            except Exception:
                pass
            self.sock = None


    def recvall(self):
        """recv until no data outstanding on the socket"""
        self.sock.setblocking(False)
        data = ''
        while True:
            try:
                seg = self.sock.recv(self.recv_size)
                if seg == '':
                    self.close()
                else:
                    data += seg
            except (IOError, socket.error) as e:
                err = e.args[0]
                if err == errno.EINTR:
                    continue
                if err == errno.EAGAIN or err == errno.EWOULDBLOCK:
                    if len(data) == 0:
                        continue
                    else:
                        break
                else:
                    raise
        self.sock.setblocking(True)
        return data

    def send(self, data):
        """send req until all data is sent, or an error occurs"""
        if not self.sock:
            self.connect()
        try:
            self.sock.sendall(data)
        except Exception:
            self.close()
            raise


class DataClient(TCPClient):
    DELIM = '\r\n'

    def request(self, req):
        """send a (multi-line) request, req should be of a sequence type"""
        for line in req:
            self.send(line + DataClient.DELIM)

    def response(self):
        """receive a response, which will be split by delimiter (retained)"""
        buf = self.recvall()
        rsp = buf.split(DataClient.DELIM)
        if rsp[-1] == '':
            rsp.pop()
        else:
            raise Exception("response not terminated by " + DataClient.DELIM)
        return rsp


class AdminClient(TCPClient):
    DELIM = '\r\n'
    STATS_CMD = 'stats'

    def stats(self):
        self.send(AdminClient.STATS_CMD + AdminClient.DELIM)
        buf = self.recvall()
        rsp = buf.split(AdminClient.DELIM)
        if rsp[-1] == '':
            rsp.pop()
        else:
            raise Exception("response not terminated by " + DataClient.DELIM)
        if rsp[-1] == 'END':
            rsp.pop()
        else:
            raise Exception('ending not detected, found {} instead'.format(rsp[-1]))

        return dict(line.split(' ')[1:] for line in rsp)

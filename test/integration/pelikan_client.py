import errno
import socket


class PelikanClient(object):
    def __init__(self, server_port, admin_port):
        self.client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.client.connect(('localhost', server_port))
        self.admin = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.admin.connect(('localhost', admin_port))

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
        if data[-5:] != 'END\r\n':
            raise Exception('Invalid data while fetching stats: {}'.format(data))
        return dict(line.split(' ')[1:] for line in data[:-5].strip().split('\r\n'))

    def read(self, length):
        return self.client.recv(length)

    def write(self, data):
        self.client.sendall(data)

import os
import subprocess
import tempfile


class PelikanServer(object):
    @staticmethod
    def default_path():
        return os.path.realpath(os.path.join(
            os.path.dirname(os.path.abspath(__file__)),
            '../../_build/_bin'
        ))

    def __init__(self, executable):
        self.executable = executable

    def start(self, **kwargs):
        self.config_file = tempfile.NamedTemporaryFile()
        self.config_file.write('\n'.join(('{}: {}'.format(k, v) for (k, v) in kwargs.items())))
        self.config_file.flush()

        executable = os.path.join(
            os.getenv('PELIKAN_BIN_PATH', PelikanServer.default_path()),
            self.executable
        )
        self.server = subprocess.Popen((executable, self.config_file.name),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def wait_ready(self):
        # wait for the port to be listening; no great "ready" output present
        # but it lists all configs, so this must exist
        while 'name: server_port' not in self.server.stdout.readline():
            pass

    def stop(self):
        self.server.kill()

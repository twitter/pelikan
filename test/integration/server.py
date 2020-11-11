import os
import subprocess
#import tempfile


class PelikanServer(object):
  SUPPORTED_SERVER = [
    'pelikan_pingserver',
    'pelikan_rds',
    'pelikan_slimcache',
    'pelikan_twemcache' ]


  @staticmethod
  def default_path():
    return os.path.realpath(os.path.join(
      os.path.dirname(os.path.abspath(__file__)),
      '../../_build/_bin'
    ))


  def __init__(self, executable, config=None):
    if executable not in PelikanServer.SUPPORTED_SERVER:
      raise Exception('executable not supported')
    self.executable = executable
    self.config = config
    if config:  # server different from default
      # TODO: find out server info from config
      pass
    self.start(config)


  def start(self, config):
    executable = os.path.join(
      os.getenv('PELIKAN_BIN_PATH', PelikanServer.default_path()),
      self.executable
    )
    exec_tup = (executable, self.config) if self.config else (executable)

    self.server = subprocess.Popen(exec_tup,
      stdin=subprocess.PIPE,
      stdout=subprocess.PIPE,
      stderr=subprocess.PIPE,
    )


  def ready(self):
    # wait for the port to be listening; no great "ready" output present
    # but it lists all configs, so this must exist
    if not self.server:
      raise Exception('server is not started')
    line = self.server.stdout.readline()
    while not line.decode('UTF-8').startswith(u'name: server_port'):
      line = self.server.stdout.readline()
      #pass
    print("server is up and running")


  def stop(self):
    self.server.kill()

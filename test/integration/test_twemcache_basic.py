from .base import TwemcacheTest

class TwemcacheBasicTest(TwemcacheTest):
    def test_miss(self):
        self.client.sendall('get foo\r\n')
        self.assertRead('END')
        self.assertMetrics(('request_parse', 1), ('get', 1), ('get_key_miss', 1))

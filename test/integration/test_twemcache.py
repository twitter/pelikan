from base import GenericTest

from os import listdir
import sys
import unittest


def twemcache():
  suite = unittest.TestSuite()
  for fname in listdir('twemcache'):
    test = GenericTest('pelikan_twemcache')
    test.load('twemcache/' + fname)
    suite.addTest(test)

  return suite


if __name__ == '__main__':
  result = unittest.TextTestRunner(verbosity=2).run(twemcache())
  if result.wasSuccessful():
    sys.exit(0)
  sys.exit(1)

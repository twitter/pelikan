from base import GenericTest

from os import listdir
import unittest


def twemcache():
    suite = unittest.TestSuite()
    for fname in listdir('twemcache'):
        test = GenericTest()
        test.load('twemcache/' + fname)
        suite.addTest(test)

    return suite


if __name__ == '__main__':
    unittest.TextTestRunner(verbosity=2).run(twemcache())

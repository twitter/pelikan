import sys
import os

project = u'Cache Common'
description = u"A C library for building extremely fast cache servers, clients and proxies. http://go/ccommon"
copyright = u'Twitter'

extensions = [
    'sphinx.ext.autodoc',
    'sphinx.ext.intersphinx',
    'sphinx.ext.ifconfig',
]

exclude_patterns = ['_build']
html_static_path = ['_static']

source_suffix = '.rst'
master_doc = 'index'

language = u'C'

today_fmt = '%Y/%m/%d'
pygments_style = 'sphinx'
html_theme = "default"
html_logo = u'_static/img/white_pelican.jpg'

intersphinx_mapping = {'http://docs.python.org/': None}

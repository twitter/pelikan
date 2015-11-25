*Pelikan* is Twitter's unified cache backend.

## Origins
The Twitter Cache team started working on a fork of Memcached in 2010, and currently owns several cache backends such as *redis*, *fatcache*, and  *slimcache*. These projects are highly similar in their main functionality and architecture, so we built a common framework to incorporate the features provided by *all of them*. With a highly modular architecture, we will get to keep most of the reusable parts as we iterate, improving existing functionalities and introducing new features and/or protocols in the near future.

## Dependencies
To compile and run tests, you will have to install `cmake` and `check`, a C unit test framework.

### OS X

```sh
brew install cmake check
```

### Ubuntu

```sh
apt-get install cmake check
```

## Build
To build:
```sh
mkdir _build
cd _build
cmake ..
make -j
make test
# executables can be found at $(topdir)/_bin/*
```

Please read README.cmake for more information.

## Documentation
We are actively working on documentation using Sphinx. Current source is included under `docs/`

## License
This software is licensed under the Apache 2 license, see LICENSE for details.

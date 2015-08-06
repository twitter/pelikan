# CCommon

*CCommon* is a common C library for the various cache backends developed by Twitter's cache team. For a list of projects using ccommon, visit: .

## Origins
The Twitter Cache team started working on a fork of Memcached in 2010, and over time has written various cache backends such as *fatcache*, *slimcache* and cache middle layer *twemproxy*. These projects have a lot in common, especially when you examine the project structure and the underlying mechanism that drives the runtime. Instead of stretching our effort thin by maintaining several individual code bases, we started building a library that captures the commonality of these projects. It is also our belief that the commonality extends beyond just caching, and can be used as the skeleton of writing many more high-throughput, low-latency services used intended for a distributed environment.

## Dependencies

## Build using CMake
To use cmake, make sure you already have it installed and the version is above 2.8
```bash
# you can also configure and compile in-source, i.e., directly at the project top level, but out-of-source compile is strongly encouraged by CMake.
# For one: there won't be something like "make (dist)clean" to help you clean up the mess afterwards
mkdir _build
cd _build
cmake ..
make
```

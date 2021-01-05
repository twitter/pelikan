// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use pelikan_twemcache_rs::TwemcacheBuilder;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn get_benchmark(c: &mut Criterion) {
    // launch the server
    let server = TwemcacheBuilder::new(None).spawn();

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    // connect and initialize an empty buffer
    let mut stream = TcpStream::connect("127.0.0.1:12321").expect("failed to connect");
    let mut buffer = vec![0; 4096];

    // define a benchmarking group and duration
    let mut group = c.benchmark_group("request");
    group.measurement_time(Duration::from_secs(60));
    group.throughput(Throughput::Elements(1));

    // TODO(bmartin): see about driving the various lengths with a loop so we
    // don't have to hardcode benchmarks for each pair of k-v sizes and request
    // types

    // benchmark getting empty value for a 1b key
    group.bench_function("get/1b/0b", |b| {
        b.iter(|| {
            assert!(stream.write(b"get 0\r\n").is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // assert!(stream.write(b"set 0 0 0 1\r\n0\r\n").is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response: {:?}", &buffer[0..bytes]);
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/1b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 1\r\n0\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream.write(b"set 0 0 0 2\r\n01\r\n").is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/2b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 2\r\n01\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream.write(b"set 0 0 0 4\r\n0123\r\n").is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/4b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 4\r\n0123\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream.write(b"set 0 0 0 8\r\n01234567\r\n").is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/8b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 8\r\n01234567\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream
    //     .write(b"set 0 0 0 16\r\n0123456789ABCDEF\r\n")
    //     .is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/16b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 16\r\n0123456789ABCDEF\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream
    //     .write(b"set 0 0 0 32\r\n0123456789ABCDEF0123456789ABCDEF\r\n")
    //     .is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/32b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes]
    //                 != b"VALUE 0 0 32\r\n0123456789ABCDEF0123456789ABCDEF\r\nEND\r\n"
    //             {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream
    //     .write(
    //         b"set 0 0 0 64\r\n0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\r\n"
    //     )
    //     .is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/64b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 64\r\n0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // assert!(stream.write(b"set 0 0 0 128\r\n0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\r\n").is_ok());
    // if let Ok(bytes) = stream.read(&mut buffer) {
    //     if &buffer[0..bytes] != b"STORED\r\n" {
    //         panic!("invalid response");
    //     }
    // } else {
    //     panic!("read error");
    // }

    // group.bench_function("get/1b/128b", |b| {
    //     b.iter(|| {
    //         assert!(stream.write(b"get 0\r\n").is_ok());
    //         if let Ok(bytes) = stream.read(&mut buffer) {
    //             if &buffer[0..bytes] != b"VALUE 0 0 128\r\n0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\r\nEND\r\n" {
    //                 panic!("invalid response");
    //             }
    //         } else {
    //             panic!("read error");
    //         }
    //     })
    // });

    // benchmark getting empty value for a 8b key
    group.bench_function("get/8b/0b", |b| {
        b.iter(|| {
            assert!(stream.write(b"get 01234567\r\n").is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // benchmark getting empty value for a 16b key
    group.bench_function("get/16b/0b", |b| {
        b.iter(|| {
            assert!(stream.write(b"get 0123456789ABCDEF\r\n").is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // benchmark getting empty value for a 32b key
    group.bench_function("get/32b/0b", |b| {
        b.iter(|| {
            assert!(stream
                .write(b"get 0123456789ABCDEF0123456789ABCDEF\r\n")
                .is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // benchmark getting empty value for a 64b key
    group.bench_function("get/64b/0b", |b| {
        b.iter(|| {
            assert!(stream
                .write(b"get 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\r\n")
                .is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // benchmark getting empty value for a 64b key
    group.bench_function("get/128b/0b", |b| {
        b.iter(|| {
            assert!(stream.write(b"get 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\r\n").is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // benchmark getting empty value for a 128b key
    group.bench_function("get/255b/0b", |b| {
        b.iter(|| {
            assert!(stream.write(b"get 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDE\r\n").is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"END\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // shutdown the server
    server.shutdown();
}

criterion_group!(benches, get_benchmark);
criterion_main!(benches);

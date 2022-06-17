// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![no_main]
use libfuzzer_sys::fuzz_target;

use core::time::Duration;

use seg::*;

const DEBUG_PRINTS: bool = false;

const SEG_SIZE: i32 = 1024; // 1KB
const HEAP_SIZE: usize = 8 * 1024; // 8KB => 8 segments
const HASH_POWER: u8 = 5;
const OVERFLOW_FACTOR: f64 = 1.0;

fuzz_target!(|data: &[u8]| {
    // println!("----------");
    let mut cache = Seg::builder()
        .segment_size(SEG_SIZE)
        .heap_size(HEAP_SIZE)
        .hash_power(HASH_POWER)
        .overflow_factor(OVERFLOW_FACTOR)
        .build()
        .expect("failed to create cache");

    let mut i = 0;

    while i < data.len() {
        let op = data[i];
        i += 1;

        match op {
            0 => {
                if i >= data.len() {
                    return;
                }

                let klen = data[i] as usize;
                if klen == 0 {
                    return;
                }
                if i + klen >= data.len() {
                    return;
                }

                let key = &data[i..(i + klen)];
                i += klen;

                if i >= data.len() {
                    return;
                }

                let vlen = data[i] as usize;
                if i + vlen >= data.len() {
                    return;
                }

                let value = &data[i..(i + vlen)];
                i += vlen;

                if i >= data.len() {
                    return;
                }

                let ttl = data[i] as u32;
                i += 1;

                if DEBUG_PRINTS {
                    println!(
                        "let _ = cache.insert(&{:?}, &{:?}, None, Duration::from_secs({}));",
                        key, value, ttl
                    );
                }
                let _ = cache.insert(key, value, None, Duration::from_secs(ttl.into()));
            }
            1 => {
                if i >= data.len() {
                    return;
                }

                let klen = data[i] as usize;
                if klen == 0 {
                    return;
                }
                if i + klen >= data.len() {
                    return;
                }

                let key = &data[i..(i + klen)];
                i += klen;

                if DEBUG_PRINTS {
                    println!("let _ = cache.delete(&{:?});", key);
                }
                cache.delete(key);
            }
            2 => {
                if DEBUG_PRINTS {
                    println!("let _ = cache.expire()");
                }
                let _ = cache.expire();
            }
            3 => {
                if DEBUG_PRINTS {
                    println!("let _ = cache.clear();");
                }
                let _ = cache.clear();
            }
            4 => {
                if i >= data.len() {
                    return;
                }
                let klen = data[i] as usize;
                if klen == 0 {
                    return;
                }
                if i + klen >= data.len() {
                    return;
                }

                let key = &data[i..(i + klen)];
                i += klen;

                if DEBUG_PRINTS {
                    println!("let _ = cache.get(&{:?});", key);
                }
                cache.get(key);
            }
            _ => {
                return;
            }
        }
    }
});

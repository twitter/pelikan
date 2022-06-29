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
                // insert
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
            1 => { // delete
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
            2 => { // expire
                if DEBUG_PRINTS {
                    println!("let _ = cache.expire()");
                }
                let _ = cache.expire();
            }
            3 => { // clear
                if DEBUG_PRINTS {
                    println!("let _ = cache.clear();");
                }
                let _ = cache.clear();
            }
            4 => { // get
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
            5 => { // cas
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

                if i + 4 >= data.len() {
                    return;
                }

                let cas = u32::from_be_bytes([data[i], data[i+1], data[i+2], data[i + 3]]);
                i += 4;

                if DEBUG_PRINTS {
                    println!(
                        "let _ = cache.cas(&{:?}, &{:?}, None, Duration::from_secs({}), cas);",
                        key, value, ttl
                    );
                }
                let _ = cache.cas(key, value, None, Duration::from_secs(ttl.into()), cas);
            }
            6 => { // incr
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

                if i + 8 >= data.len() {
                    return;
                }

                let value = u64::from_be_bytes([data[i], data[i+1], data[i+2], data[i + 3], data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
                i += 8;

                if DEBUG_PRINTS {
                    println!("let _ = cache.wrapping_add(&{:?}, {});", key, value);
                }
                let _ = cache.wrapping_add(key, value);
            }
            7 => { // decr
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

                if i + 8 >= data.len() {
                    return;
                }

                let value = u64::from_be_bytes([data[i], data[i+1], data[i+2], data[i + 3], data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
                i += 8;

                if DEBUG_PRINTS {
                    println!("let _ = cache.saturating_sub(&{:?}, {});", key, value);
                }
                let _ = cache.saturating_sub(key, value);
            }
            8 => { // insert numeric
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

                if i + 8 >= data.len() {
                    return;
                }

                let value = u64::from_be_bytes([data[i], data[i+1], data[i+2], data[i + 3], data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
                i += 8;

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
            9 => { // cas numeric
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

                if i + 8 >= data.len() {
                    return;
                }

                let value = u64::from_be_bytes([data[i], data[i+1], data[i+2], data[i + 3], data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
                i += 8;

                if i >= data.len() {
                    return;
                }

                let ttl = data[i] as u32;
                i += 1;

                if i + 4 >= data.len() {
                    return;
                }

                let cas = u32::from_be_bytes([data[i], data[i+1], data[i+2], data[i + 3]]);
                i += 4;

                if DEBUG_PRINTS {
                    println!(
                        "let _ = cache.cas(&{:?}, &{:?}, None, Duration::from_secs({}), cas);",
                        key, value, ttl
                    );
                }
                let _ = cache.cas(key, value, None, Duration::from_secs(ttl.into()), cas);
            }
            _ => {
                return;
            }
        }
    }
});

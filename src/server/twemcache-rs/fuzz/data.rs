// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate afl;

use bytes::BytesMut;

fn main() {
	let mut buffer = BytesMut::new();
    fuzz!(|data: &[u8]| {
        if let Ok(s) = std::str::from_utf8(data) {
        	buffer.extend_from_slice(data);
            let _ = pelikan_twemcache_rs::protocol::data::parse(&mut buffer);
            buffer.clear();
        }
    });
}

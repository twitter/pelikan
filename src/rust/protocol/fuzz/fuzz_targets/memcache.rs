// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol::Parse;
use protocol::memcache::MemcacheRequest;

pub const SPACE: u8 = 32;

fuzz_target!(|data: &[u8]| {
    if let Ok(request) = MemcacheRequest::parse(data) {
        match request.into_inner() {
            MemcacheRequest::Get { keys } | MemcacheRequest::Gets { keys } => {
                for key in keys.iter() {
                    assert!(!key.contains(&SPACE));
                    assert!(!key.is_empty());
                }
            }
            MemcacheRequest::Set { entry, .. } | MemcacheRequest::Add { entry, .. } | MemcacheRequest::Replace { entry, .. }=> {
                assert!(!entry.key().contains(&SPACE));
                assert!(!entry.key().is_empty());
            }
            MemcacheRequest::Cas { entry, .. } => {
                assert!(!entry.key().contains(&SPACE));
                assert!(!entry.key().is_empty());
            }
            MemcacheRequest::Delete { key, .. } => {
                assert!(!key.contains(&SPACE));
                assert!(!key.is_empty());
            }
        }
    }
});

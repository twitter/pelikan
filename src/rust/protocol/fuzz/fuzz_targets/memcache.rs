#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol::Parse;
use protocol::memcache::MemcacheRequest;

fuzz_target!(|data: &[u8]| {
    MemcacheRequest::parse(data);
});

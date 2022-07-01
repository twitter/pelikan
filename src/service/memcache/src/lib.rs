// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Provides an abstraction for Memcache client and server services. See the
//! protocol reference for more details about the behaviors.
//!
//! https://github.com/memcached/memcached/blob/master/doc/protocol.txt

#[macro_use]
extern crate logger;

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
pub use client::*;

#[cfg(feature = "server")]
pub use server::*;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

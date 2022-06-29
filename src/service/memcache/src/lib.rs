// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Provides an abstraction for Memcache client and server services.

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "server")]
mod server;


#[cfg(feature = "client")]
pub use client::*;

#[cfg(feature = "server")]
pub use server::*;

use rustcommon_metrics::{metric, Counter};

// the stuff below can move into a common crate

#[macro_export]
#[rustfmt::skip]
macro_rules! counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

#[cfg(not(any(feature = "namespace", all(feature = "client", feature = "server"))))]
pub use counter as client_counter;

#[cfg(any(feature = "namespace", all(feature = "client", feature = "server")))]
#[macro_export]
#[rustfmt::skip]
macro_rules! client_counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name,
            namespace = "client"
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            namespace = "client",
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

#[cfg(not(any(feature = "namespace", all(feature = "client", feature = "server"))))]
pub use counter as server_counter;

#[cfg(any(feature = "namespace", all(feature = "client", feature = "server")))]
#[macro_export]
#[rustfmt::skip]
macro_rules! server_counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name,
            namespace = "server"
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            namespace = "server",
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::PingStorage;
use crate::*;

// TODO(bmartin): we currently don't have pingserver specific metrics. Once we
// have a better way of handling distributed metrics regisitry we should enable
// pingserver specific stats and ensure we don't get segcache stats exported in
// the pingserver. For now, we are prioritizing segcache stats.

// use metrics::Stat;

impl<'a, T> Execute<PingRequest, PingResponse> for T
where
    T: PingStorage,
{
    fn execute(&mut self, request: PingRequest) -> Option<PingResponse> {
        let response = match request {
            PingRequest::Ping => {
                // increment_counter!(&Stat::Ping);

                PingResponse::Pong
            }
        };

        Some(response)
    }
}

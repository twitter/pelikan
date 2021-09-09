// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::PingStorage;
use crate::*;

use metrics::{static_metrics, Counter};

static_metrics! {
    static PING: Counter;
}

impl<'a, T> Execute<PingRequest, PingResponse> for T
where
    T: PingStorage,
{
    fn execute(&mut self, request: PingRequest) -> Option<PingResponse> {
        let response = match request {
            PingRequest::Ping => {
                PING.increment();

                PingResponse::Pong
            }
        };

        Some(response)
    }
}

// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Allows for `Noop` storage to be used for the `Ping` protocol.

use super::*;

use protocol_ping::*;

impl PingStorage for Noop {}

impl Execute<PingRequest, PingResponse> for Noop {
    fn execute(&mut self, request: PingRequest) -> Option<PingResponse> {
        let response = match request {
            PingRequest::Ping => PingResponse::Pong,
        };

        Some(response)
    }
}

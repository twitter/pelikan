// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Allows for `Noop` storage to be used for the `Ping` protocol.

use super::*;

use protocol_ping::*;

impl PingStorage for Noop {}

impl Execute<Request, Response> for Noop {
    fn execute(&mut self, request: Request) -> ExecutionResult<Request, Response> {
        let response = match request {
            Request::Ping => Response::Pong,
        };

        ExecutionResult::new(request, response)
    }
}

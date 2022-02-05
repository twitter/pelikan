// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Allows for `Noop` storage to be used for the `Ping` protocol.

use super::*;

use protocol::ping::PingStorage;

impl PingStorage for Noop {}

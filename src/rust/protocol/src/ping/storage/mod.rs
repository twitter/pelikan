// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines what operations that a storage implementation must
//! implement to be used as storage for the `Ping` protocol.
//!
//! In the case of the `Ping` protocol, storage is effectively no-op so the
//! trait has no functions.

/// Defines operations that arbitrary storage must be able to handle to be used
/// as storage for a ping server.
pub trait PingStorage {}

// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the serialization of `Ping` protocol responses into the wire
//! representation.

mod compose;
mod keyword;
mod parse;

#[cfg(test)]
mod test;

pub use parse::Parser as ResponseParser;

/// A collection of all possible `Ping` responses
pub enum Response {
    Pong,
}

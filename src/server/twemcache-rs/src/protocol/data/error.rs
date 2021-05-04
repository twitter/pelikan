// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use thiserror::Error;

#[derive(Error, PartialEq, Eq, Debug)]

pub enum ParseError {
	#[error("incomplete request")]
    Incomplete,
    #[error("invalid request")]
    Invalid,
    #[error("unknown command")]
    UnknownCommand,
}
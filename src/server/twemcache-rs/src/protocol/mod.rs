// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// TODO(bmartin): probably makes sense to have a unifying trait here.

pub mod admin;
pub mod data;

pub const CRLF: &[u8] = b"\r\n";
pub const CRLF_LEN: usize = CRLF.len();

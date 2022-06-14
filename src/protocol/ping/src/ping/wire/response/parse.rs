// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module handles parsing of the wire representation of a `Ping` request
//! into a request object.

use super::super::*;
use crate::ping::wire::response::keyword::Keyword;
use crate::*;

use core::slice::Windows;
use std::convert::TryFrom;

#[derive(Default, Copy, Clone)]
pub struct Parser {}

impl Parser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parse<Response> for Parser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Response>, ParseError> {
        match parse_keyword(buffer)? {
            Keyword::Pong => parse_pong(buffer),
        }
    }
}

struct ParseState<'a> {
    single_byte: Windows<'a, u8>,
    double_byte: Windows<'a, u8>,
}

impl<'a> ParseState<'a> {
    fn new(buffer: &'a [u8]) -> Self {
        let single_byte = buffer.windows(1);
        let double_byte = buffer.windows(2);
        Self {
            single_byte,
            double_byte,
        }
    }

    fn next_space(&mut self) -> Option<usize> {
        self.single_byte.position(|w| w == b" ")
    }

    fn next_crlf(&mut self) -> Option<usize> {
        self.double_byte.position(|w| w == CRLF.as_bytes())
    }
}

fn parse_keyword(buffer: &[u8]) -> Result<Keyword, ParseError> {
    let command;
    {
        let mut parse_state = ParseState::new(buffer);
        if let Some(line_end) = parse_state.next_crlf() {
            if let Some(cmd_end) = parse_state.next_space() {
                command = Keyword::try_from(&buffer[0..cmd_end])?;
            } else {
                command = Keyword::try_from(&buffer[0..line_end])?;
            }
        } else {
            return Err(ParseError::Incomplete);
        }
    }
    Ok(command)
}

#[allow(clippy::unnecessary_wraps)]
fn parse_pong(buffer: &[u8]) -> Result<ParseOk<Response>, ParseError> {
    let mut parse_state = ParseState::new(buffer);

    // this was already checked for when determining the command
    let line_end = parse_state.next_crlf().unwrap();

    let consumed = line_end + CRLF.len();

    let message = Response::Pong;

    #[cfg(feature = "client")]
    PONG.increment();

    Ok(ParseOk::new(message, consumed))
}

// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use cc_binding::{
    option, option_type, OPTION_TYPE_BOOL, OPTION_TYPE_FPN, OPTION_TYPE_STR, OPTION_TYPE_UINT,
};

use std::borrow::Cow;
use std::error::Error;
use std::ffi::CStr;
use std::fmt;
use std::io::{BufRead, Error as IOError};

macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const i8
    };
}

/// Error for when a config file fails to parse.
///
/// This covers everything from an IO error to running out of memory
/// to a missing colon within a line.
pub struct ParseError<'a> {
    span: Span<'a>,
    data: ParseErrorType,
}

impl<'a> ParseError<'a> {
    pub fn kind(&self) -> ParseErrorKind {
        match &self.data {
            ParseErrorType::InvalidBool => ParseErrorKind::InvalidBool,
            ParseErrorType::InvalidUInt => ParseErrorKind::InvalidUInt,
            ParseErrorType::StringWithNull => ParseErrorKind::InvalidString,
            ParseErrorType::InvalidFloat => ParseErrorKind::InvalidFloat,
            ParseErrorType::NoColonInLine => ParseErrorKind::IncorrectKVSyntax,
            ParseErrorType::UnknownOptionType => ParseErrorKind::UnknownOptionType,
            ParseErrorType::OutOfMemory => ParseErrorKind::OutOfMemory,
            ParseErrorType::IOError(_) => ParseErrorKind::IOError,
            ParseErrorType::InvalidKey => ParseErrorKind::UnknownOption,
        }
    }
}

impl<'a> ParseError<'a> {
    fn invalid_bool(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::InvalidBool,
        }
    }

    fn invalid_uint(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::InvalidUInt,
        }
    }

    fn invalid_fpn(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::InvalidFloat,
        }
    }

    fn string_contained_null(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::StringWithNull,
        }
    }

    fn invalid_key(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::InvalidKey,
        }
    }

    fn missing_colon(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::NoColonInLine,
        }
    }

    fn unknown_option_type(span: Span<'a>) -> Self {
        Self {
            span,
            data: ParseErrorType::UnknownOptionType,
        }
    }

    fn ioerror(err: IOError) -> Self {
        Self {
            span: Span::new(&[], 0),
            data: ParseErrorType::IOError(err),
        }
    }

    fn oom() -> Self {
        Self {
            span: Span {
                line: 0,
                text: Cow::Borrowed(&[]),
            },
            data: ParseErrorType::OutOfMemory,
        }
    }

    fn to_owned(self) -> ParseError<'static> {
        ParseError {
            span: Span {
                line: self.span.line,
                text: Cow::Owned(self.span.text.into_owned()),
            },
            data: self.data,
        }
    }
}

#[derive(Debug)]
enum ParseErrorType {
    InvalidBool,
    InvalidUInt,
    StringWithNull,
    InvalidFloat,

    InvalidKey,
    UnknownOptionType,
    NoColonInLine,
    IOError(IOError),
    OutOfMemory,
}

/// The type of the parse error,
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ParseErrorKind {
    /// Tried to parse a bool, but it wasn't valid.
    InvalidBool,
    /// Tried to parse an unsigned integer, but it wasn't valid.
    InvalidUInt,
    /// Tried to parse a string, but it contained a nul character
    InvalidString,
    /// Tried to parse a float, but it wasn't valid.
    InvalidFloat,
    /// Tried to parse a `key: value` pair but a colon was missing.
    IncorrectKVSyntax,

    /// Parsed a valid option, but it doesn't correspond to any
    /// of the options that were provided.
    UnknownOption,
    /// An option had an unrecognized option type.
    UnknownOptionType,
    /// Was unable to allocate memory when it was needed while parsing.
    ///
    /// Note that this doesn't apply to all OOM cases.
    OutOfMemory,
    /// An IO error occurred when reading from the stream.
    IOError,
}

/// Load options from a type which implements `BufRead`.
///
/// In most cases you'll want to use [`OptionExt::load`][0]
/// instead.
///
/// [0]: crate::option::OptionExt::load
pub fn option_load<R: BufRead>(
    options: &mut [option],
    source: &mut R,
) -> Result<(), ParseError<'static>> {
    let mut linebuf = Vec::new();

    let mut lineno = 0;

    while source
        .read_until(b'\n', &mut linebuf)
        .map_err(ParseError::ioerror)?
        != 0
    {
        // Strip off any comments before doing parsing
        let line = linebuf.split(|&x| x == b'#').next().unwrap();

        if line.iter().copied().all(|x| x.is_ascii_whitespace()) {
            linebuf.clear();
            continue;
        }

        let (k, v) = parse_kv(&line, lineno).map_err(|x| x.to_owned())?;

        let opt = match unsafe { find_option(options, k) } {
            Some(opt) => opt,
            None => return Err(ParseError::invalid_key(Span::new(k, lineno)).to_owned()),
        };

        let value = parse_value(v, lineno, opt.type_).map_err(|x| x.to_owned())?;
        unsafe { set_option_value(opt, value)? };

        lineno += 1;
        linebuf.clear();
    }

    Ok(())
}

/// Load a single option from a byte string.
pub fn option_set<'a>(option: &mut option, value: &'a [u8]) -> Result<(), ParseError<'a>> {
    let value = parse_value(value, 0, option.type_)?;

    unsafe { set_option_value(option, value) }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct Span<'a> {
    text: Cow<'a, [u8]>,
    line: u32,
}

impl<'a> Span<'a> {
    pub fn new(text: &'a [u8], line: u32) -> Self {
        Self {
            text: Cow::Borrowed(text),
            line,
        }
    }
}

enum Value<'a> {
    Str(&'a [u8]),
    Float(f64),
    UInt(u64),
    Bool(bool),
}

fn is_space(c: Option<u8>) -> bool {
    c.map(|x| x.is_ascii_whitespace()).unwrap_or(false)
}

/// Trim starting and ending spaces off of a byte slice
fn trim_bytes<'a>(mut slice: &'a [u8]) -> &'a [u8] {
    while is_space(slice.first().copied()) {
        slice = slice.split_first().unwrap().1;
    }

    while is_space(slice.last().copied()) {
        slice = slice.split_last().unwrap().1
    }

    slice
}

/// Parse a key-value pair separated by `:` then strip the whitespace
/// off of both the key and the value.
fn parse_kv<'a>(input: &'a [u8], line: u32) -> Result<(&'a [u8], &'a [u8]), ParseError<'a>> {
    let mut first = true;

    let mut split = input.split(|&x| {
        if x == b':' && first {
            first = false;
            return true;
        }

        false
    });

    let key = match split.next() {
        Some(x) => x,
        // There will always be at least one subslice
        None => unreachable!(),
    };
    let value = match split.next() {
        Some(x) => x,
        None => return Err(ParseError::missing_colon(Span::new(input, line))),
    };

    let key = trim_bytes(key);

    if !is_valid_name(key) {
        return Err(ParseError::invalid_key(Span::new(key, line)));
    }

    Ok((key, trim_bytes(value)))
}

/// Validate a key name.
///
/// A valid key can only contain characters in `[a-zA-Z0-9_]`.
fn is_valid_name(key: &[u8]) -> bool {
    key.iter()
        .copied()
        .all(|x: u8| x.is_ascii_alphanumeric() || x == b'_')
}

/// Parse an unsigned integer. Does not implement expression handling
/// that was present in the original ccommon implmentation since it
/// was deemed that it probably wasn't needed.
fn parse_uint<'a>(input: &'a [u8], line: u32) -> Result<u64, ParseError<'a>> {
    use std::str;

    let s = str::from_utf8(input).map_err(|_| ParseError::invalid_uint(Span::new(input, line)))?;

    s.parse()
        .map_err(|_| ParseError::invalid_uint(Span::new(input, line)))
}

/// Parse a string value. The only thing this method does is verify that
/// the string doesn't contain any nul characters.
fn parse_str<'a>(input: &'a [u8], line: u32) -> Result<&'a [u8], ParseError<'a>> {
    if input.iter().any(|&c| c == 0) {
        return Err(ParseError::string_contained_null(Span::new(input, line)));
    }

    Ok(input)
}

/// Parse a boolean. A boolean is either the literal `yes` or the literal `no.
fn parse_bool<'a>(input: &'a [u8], line: u32) -> Result<bool, ParseError<'a>> {
    match input {
        b"yes" => Ok(true),
        b"no" => Ok(false),
        _ => Err(ParseError::invalid_bool(Span::new(input, line))),
    }
}

/// Parse a floating point number. This just calls `f64::parse`.
fn parse_fpn<'a>(input: &'a [u8], line: u32) -> Result<f64, ParseError<'a>> {
    use std::str;

    let s = str::from_utf8(input).map_err(|_| ParseError::invalid_fpn(Span::new(input, line)))?;

    s.parse()
        .map_err(|_| ParseError::invalid_fpn(Span::new(input, line)))
}

/// Given the type of the option, parse a value
fn parse_value<'a>(
    input: &'a [u8],
    line: u32,
    ty: option_type,
) -> Result<Value<'a>, ParseError<'a>> {
    match ty {
        OPTION_TYPE_UINT => Ok(Value::UInt(parse_uint(input, line)?)),
        OPTION_TYPE_BOOL => Ok(Value::Bool(parse_bool(input, line)?)),
        OPTION_TYPE_STR => Ok(Value::Str(parse_str(input, line)?)),
        OPTION_TYPE_FPN => Ok(Value::Float(parse_fpn(input, line)?)),

        _ => Err(ParseError::unknown_option_type(Span::new(input, line))),
    }
}

/// Search for a named option within the options array.
unsafe fn find_option<'a>(options: &'a mut [option], name: &[u8]) -> Option<&'a mut option> {
    for opt in options.iter_mut() {
        assert!(!opt.name.is_null());

        let opt_name = CStr::from_ptr(opt.name);

        if opt_name.to_bytes() == name {
            return Some(opt);
        }
    }

    None
}

unsafe fn set_option_value(option: &mut option, value: Value) -> Result<(), ParseError<'static>> {
    use cc_binding::{_cc_alloc, _cc_free};
    use std::mem::MaybeUninit;

    match value {
        Value::Bool(v) => option.val.vbool = v,
        Value::Float(v) => option.val.vfpn = v,
        Value::UInt(v) => option.val.vuint = v,
        Value::Str(v) => {
            if option.set && !option.val.vstr.is_null() {
                // Avoid leaking memory in the case where the option has
                // already been initialized.
                _cc_free(
                    option.val.vstr as *mut libc::c_void,
                    c_str!(module_path!()),
                    line!() as std::os::raw::c_int,
                );
            }

            let mem = _cc_alloc(
                v.len() + 1,
                c_str!(module_path!()),
                line!() as std::os::raw::c_int,
            ) as *mut MaybeUninit<u8>;

            if mem.is_null() {
                return Err(ParseError::oom());
            }

            // Copy the string over
            std::ptr::copy_nonoverlapping(v.as_ptr() as *const MaybeUninit<u8>, mem, v.len());

            // Add nul terminator
            std::ptr::write(mem.wrapping_add(v.len()), MaybeUninit::new(0));

            option.val.vstr = mem as *mut i8
        }
    }

    option.set = true;

    Ok(())
}

fn escape_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|x| std::ascii::escape_default(*x))
        .map(|c| c as char)
        .collect()
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Bool(v) => fmt.debug_tuple("Bool").field(v).finish(),
            Value::Float(v) => fmt.debug_tuple("Float").field(v).finish(),
            Value::UInt(v) => fmt.debug_tuple("UInt").field(v).finish(),
            Value::Str(s) => {
                let escaped = &escape_string(s);
                fmt.debug_tuple("Str").field(escaped).finish()
            }
        }
    }
}

impl fmt::Display for ParseError<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use ParseErrorType::*;

        let spanmsg: String = (*self.span.text)
            .into_iter()
            .copied()
            .flat_map(|c| std::ascii::escape_default(c))
            .map(|c| c as char)
            .collect();

        match &self.data {
            InvalidBool => write!(
                fmt,
                "line {}: '{}' is not a valid boolean expression, expected 'yes' or 'no'",
                self.span.line, spanmsg
            ),
            InvalidUInt => write!(
                fmt,
                "line {}: '{}' is not a valid integer",
                self.span.line, spanmsg
            ),
            StringWithNull => write!(
                fmt,
                "line {}: string literal contained nul character",
                self.span.line
            ),
            InvalidFloat => write!(
                fmt,
                "line {}: '{}' is not a valid floating point number",
                self.span.line, spanmsg
            ),
            InvalidKey => write!(
                fmt,
                "line {}: option '{}' not recognized",
                self.span.line, spanmsg
            ),
            UnknownOptionType => write!(fmt, "unknown option type"),
            NoColonInLine => write!(
                fmt,
                "line {}: invalid formatting, expected '<key>: <value>'",
                self.span.line
            ),
            IOError(e) => write!(fmt, "IO error: {}", e),
            OutOfMemory => write!(fmt, "ran out of memory while parsing file"),
        }
    }
}

impl fmt::Debug for ParseError<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        // Create a debug representation that actually shows the error
        // message since the internals are fairly unhelpful when they
        // show up in a `unwrap` or `expect` call.
        fmt.debug_struct("ParseError")
            .field("message", &format_args!("{}", self))
            .field("kind", &self.kind())
            .finish()
    }
}

impl<'a> Error for ParseError<'a> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.data {
            ParseErrorType::IOError(e) => Some(e),
            _ => None,
        }
    }
}

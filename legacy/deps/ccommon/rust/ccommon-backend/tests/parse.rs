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

use ccommon_backend::option::*;
use ccommon_rs::{option::*, Options};

use std::ffi::CStr;
use std::io::Cursor;

#[derive(Options)]
#[repr(C)]
pub struct TestOptions {
    #[option(desc = "float option")]
    fpn_option: Float,
    #[option(desc = "uint option")]
    uint_option: UInt,
    #[option(desc = "bool option")]
    bool_option: Bool,
    #[option(desc = "string option")]
    str_option: Str,
}

macro_rules! c_str_ffi {
    ($s:expr) => {
        unsafe { CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes()) }
    };
}

fn cstr_to_str(ptr: *const libc::c_char) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }

    unsafe {
        let cstr = CStr::from_ptr(ptr);
        Some(cstr.to_str().unwrap())
    }
}

#[test]
fn parse_sanity_test() {
    let mut options = TestOptions::new();
    let config: &[u8] = br#"
            fpn_option: 53.1
            uint_option: 1000
            bool_option: yes
            str_option: Hello, World!
        "#;

    let mut cursor = Cursor::new(config);
    options.load(&mut cursor).expect("Failed to parse config");

    assert_eq!(options.fpn_option.value(), 53.1);
    assert_eq!(options.uint_option.value(), 1000);
    assert_eq!(options.bool_option.value(), true);
    assert_eq!(
        options.str_option.as_cstr(),
        Some(c_str_ffi!("Hello, World!"))
    );
}

#[test]
fn unknown_option() {
    let mut options = TestOptions::new();
    let config: &[u8] = b"unknown: FOO";

    let mut cursor = Cursor::new(config);
    let err = options.load(&mut cursor).unwrap_err();

    assert_eq!(err.kind(), ParseErrorKind::UnknownOption);
}

#[test]
fn parse_bool() {
    let mut options = TestOptions::new();
    let config1: &[u8] = b"bool_option: yes";
    let config2: &[u8] = b"bool_option: no";

    let mut cursor1 = Cursor::new(config1);
    options.load(&mut cursor1).unwrap();

    assert_eq!(options.bool_option.value(), true);

    let mut cursor2 = Cursor::new(config2);
    options.load(&mut cursor2).unwrap();

    assert_eq!(options.bool_option.value(), false);
}

#[test]
fn overlarge_uint() {
    let mut options = TestOptions::new();
    let config: &[u8] = b"uint_option: 100000000000000000000000000000000000000000";

    let mut cursor = Cursor::new(config);
    let err = options.load(&mut cursor).unwrap_err();

    assert_eq!(err.kind(), ParseErrorKind::InvalidUInt);
}

#[test]
fn non_colon_separated_line() {
    let mut options = TestOptions::new();
    let config: &[u8] = b"foobar";

    let mut cursor = Cursor::new(config);
    let err = options.load(&mut cursor).unwrap_err();

    assert_eq!(err.kind(), ParseErrorKind::IncorrectKVSyntax);
}

#[test]
fn test_parse_uint() {
    let mut options = TestOptions::new();

    let mut cursor = Cursor::new(b"uint_option: invalid");
    assert!(options.load(&mut cursor).is_err());

    let mut cursor = Cursor::new(b"uint_option: -1");
    assert!(options.load(&mut cursor).is_err());

    let mut cursor = Cursor::new(b"uint_option: 1");
    options.load(&mut cursor).unwrap();
    assert_eq!(options.uint_option.value(), 1);

    let mut cursor = Cursor::new(b"uint_option: 1A");
    assert!(options.load(&mut cursor).is_err());
}

#[test]
fn test_parse_float() {
    let mut options = TestOptions::new();

    let mut cursor = Cursor::new(b"fpn_option: invalid");
    assert!(options.load(&mut cursor).is_err());

    let mut cursor = Cursor::new(b"fpn_option: 1.0e-5X");
    assert!(options.load(&mut cursor).is_err());

    let mut cursor = Cursor::new(&b"fpn_option: 1.0e10000000000000000000000"[..]);
    options.load(&mut cursor).unwrap();
    assert!(options.fpn_option.value().is_infinite());

    let mut cursor = Cursor::new("fpn_option: 1.0");
    options.load(&mut cursor).unwrap();
    assert_eq!(options.fpn_option.value(), 1.0);

    let mut cursor = Cursor::new("fpn_option: -1e20");
    options.load(&mut cursor).unwrap();
    assert_eq!(options.fpn_option.value(), -1e20);
}

#[test]
fn test_parse_string() {
    let mut options = TestOptions::new();

    let mut cursor = Cursor::new(b"str_option: foo");
    options.load(&mut cursor).unwrap();
    assert_eq!(cstr_to_str(options.str_option.value()), Some("foo"));

    let mut cursor = Cursor::new(b"str_option: foo\0bar");
    assert!(options.load(&mut cursor).is_err());
}

#[test]
fn test_parse_comments() {
    let mut options = TestOptions::new();

    let mut cursor = Cursor::new(
        &br#"
        # Test Comment
        uint_option: 22
        # uint_option: 34
    "#[..],
    );
    options.load(&mut cursor).unwrap();
    assert_eq!(options.uint_option.value(), 22);
}

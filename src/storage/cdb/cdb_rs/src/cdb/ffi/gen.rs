#![allow(unknown_lints)]
#![allow(clippy)]
#![allow(clippy_pedantic)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

// TODO(sean): fix the build script here so that it only generates the necessary bindings
// Note: This is a hack to get CI to pass, otherwise the warnings
// are so long that travis fails the build due to excessive output.
#![allow(improper_ctypes)]

include!(concat!(env!("OUT_DIR"), "/ffigen.rs"));

#![cfg(feature = "derive")]

use ccommon_rs::option::*;
use ccommon_rs::Options;

#[derive(Options)]
#[repr(C)]
pub struct TestMetrics {
    #[option(desc = "A test gauge metric")]
    pub m1: Float,
    #[option(desc = "A test counter metric")]
    pub m2: Bool,
    #[option(desc = "A test fpn metric", name = "test.other.m3")]
    pub m3: UInt,
}

#[derive(Options)]
#[repr(C)]
pub struct Nested {
    pub inner: TestMetrics,
    #[option(
        desc = "Another test metric",
        name = "nested.other",
        default = std::ptr::null_mut()
    )]
    other: Str,
    #[option(desc = "Defaulted str")]
    defaulted: Str,
}

#[derive(Options)]
#[repr(transparent)]
pub struct Tuple(Nested);

#[derive(Options)]
#[repr(C, align(16))]
pub struct Marker;

macro_rules! c_str {
    ($s:expr) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes()) }
    };
}

#[test]
fn this_compiles() {
    let nested = Nested::new();

    assert_eq!(nested.other.name(), c_str!("nested.other"));
    assert_eq!(nested.inner.m3.name(), c_str!("test.other.m3"));
    assert_eq!(nested.inner.m2.name(), c_str!("m2"));
    assert_eq!(nested.other.value(), nested.other.default());
    assert_eq!(nested.other.value(), std::ptr::null_mut());
}

#![cfg(feature = "derive")]

use ccommon_rs::metric::*;
use ccommon_rs::Metrics;

#[derive(Metrics)]
#[repr(C)]
pub struct TestMetrics {
    #[metric(desc = "A test gauge metric")]
    pub m1: Gauge,
    #[metric(desc = "A test counter metric")]
    pub m2: Counter,
    #[metric(desc = "A test fpn metric", name = "test.other.m3")]
    pub m3: Fpn,
}

#[derive(Metrics)]
#[repr(C)]
pub struct Nested {
    pub inner: TestMetrics,
    #[metric(desc = "Another test metric", name = "nested.other")]
    other: Gauge,
}

#[derive(Metrics)]
#[repr(transparent)]
pub struct Tuple(Nested);

#[derive(Metrics)]
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
}

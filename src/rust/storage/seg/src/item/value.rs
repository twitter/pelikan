// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A mostly opaque representation of an item's value to allow a it to carry
//! type information.

use std::io::Write;

pub struct Value<'a> {
    pub(crate) inner: TypedValue<'a>,
}

#[derive(PartialEq)]
pub enum TypedValue<'a> {
    Bytes(&'a [u8]),
    U64(u64),
    OwnedBytes(Box<[u8]>),
}

impl Value<'_> {
    /// The length of the value in bytes for the internal representation
    pub(crate) fn packed_len(&self) -> usize {
        match self.inner {
            TypedValue::Bytes(v) => v.len(),
            TypedValue::OwnedBytes(ref v) => v.len(),
            TypedValue::U64(_) => core::mem::size_of::<u64>(),
        }
    }

    /// The length of the value in bytes as it would be in a serialized format.
    ///
    /// *NOTE*: numeric types are serialized in a string format and not as the
    /// raw byte representation. This means that these types will be variable
    /// length.
    pub fn len(&self) -> usize {
        match self.inner {
            TypedValue::Bytes(v) => v.len(),
            TypedValue::OwnedBytes(ref v) => v.len(),
            TypedValue::U64(v) => format!("{}", v).as_bytes().len(),
        }
    }

    /// Returns true if the value has no size. For example, if the value is a
    /// byte slice with zero length.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Writes the value to the provided writer. This can be used to serialize
    /// the value onto the wire.
    ///
    /// *NOTE*: numeric types are serialized in a string format and not as the
    /// raw byte representation. This means that these types will be variable
    /// length.
    pub fn write_all_to<W: Write>(&self, mut dst: W) -> Result<(), std::io::Error> {
        match self.inner {
            TypedValue::Bytes(v) => dst.write_all(v),
            TypedValue::OwnedBytes(ref v) => dst.write_all(v),
            TypedValue::U64(v) => dst.write_all(format!("{}", v).as_bytes()),
        }
    }
}

impl<'a> core::fmt::Debug for Value<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.inner {
            TypedValue::Bytes(v) => write!(f, "{:?}", v),
            TypedValue::OwnedBytes(v) => write!(f, "{:?}", v),
            TypedValue::U64(v) => write!(f, "{:?}", v),
        }
    }
}

impl<'a, const N: usize> From<[u8; N]> for Value<'a> {
    fn from(other: [u8; N]) -> Self {
        Value {
            inner: TypedValue::OwnedBytes(Box::new(other)),
        }
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for Value<'a> {
    fn from(other: &'a [u8; N]) -> Self {
        Value {
            inner: TypedValue::Bytes(&other[0..N]),
        }
    }
}

impl<'a> From<Box<[u8]>> for Value<'a> {
    fn from(other: Box<[u8]>) -> Self {
        Value {
            inner: TypedValue::OwnedBytes(other),
        }
    }
}

impl<'a> From<&'a [u8]> for Value<'a> {
    fn from(other: &'a [u8]) -> Self {
        Value {
            inner: TypedValue::Bytes(other),
        }
    }
}

impl<'a> From<Vec<u8>> for Value<'a> {
    fn from(other: Vec<u8>) -> Self {
        Value {
            inner: TypedValue::OwnedBytes(other.into_boxed_slice()),
        }
    }
}

impl<'a> From<&'a Vec<u8>> for Value<'a> {
    fn from(other: &'a Vec<u8>) -> Self {
        Value {
            inner: TypedValue::Bytes(other),
        }
    }
}

impl<'a> From<u64> for Value<'a> {
    fn from(other: u64) -> Self {
        Value {
            inner: TypedValue::U64(other),
        }
    }
}

impl<'a> PartialEq<[u8]> for Value<'a> {
    fn eq(&self, rhs: &[u8]) -> bool {
        match self.inner {
            TypedValue::Bytes(v) => v == rhs,
            TypedValue::OwnedBytes(ref v) => v.as_ref() == rhs,
            TypedValue::U64(_) => false,
        }
    }
}

impl<'a, const N: usize> PartialEq<[u8; N]> for Value<'a> {
    fn eq(&self, rhs: &[u8; N]) -> bool {
        match self.inner {
            TypedValue::Bytes(v) => v == rhs,
            TypedValue::OwnedBytes(ref v) => v.as_ref() == rhs,
            TypedValue::U64(_) => false,
        }
    }
}

impl<'a, const N: usize> PartialEq<&[u8; N]> for Value<'a> {
    fn eq(&self, rhs: &&[u8; N]) -> bool {
        match self.inner {
            TypedValue::Bytes(v) => v == *rhs,
            TypedValue::OwnedBytes(ref v) => v.as_ref() == *rhs,
            TypedValue::U64(_) => false,
        }
    }
}

impl<'a> PartialEq<u64> for Value<'a> {
    fn eq(&self, rhs: &u64) -> bool {
        match self.inner {
            TypedValue::Bytes(_) => false,
            TypedValue::OwnedBytes(_) => false,
            TypedValue::U64(v) => v == *rhs,
        }
    }
}

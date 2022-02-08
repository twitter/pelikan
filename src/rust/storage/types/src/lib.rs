// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[derive(PartialEq)]
pub enum Value<'a> {
    Bytes(&'a [u8]),
    U64(u64),
}

#[derive(PartialEq)]
pub enum OwnedValue {
    Bytes(Box<[u8]>),
    U64(u64),
}

impl<'a> Value<'a> {
    pub fn to_owned(&self) -> OwnedValue {
        match self {
            Self::Bytes(v) => OwnedValue::Bytes(v.to_vec().into_boxed_slice()),
            Self::U64(v) => OwnedValue::U64(*v),
        }
    }
}

impl<'a> OwnedValue {
    pub fn as_value(&'a self) -> Value<'a> {
        match self {
            Self::Bytes(v) => Value::Bytes(v.as_ref()),
            Self::U64(v) => Value::U64(*v),
        }
    }
}

impl From<u64> for Value<'_> {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

impl<'a> From<&'a [u8]> for Value<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::Bytes(value)
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(value: &'a str) -> Self {
        Self::Bytes(value.as_bytes())
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for Value<'a> {
    fn from(value: &'a [u8; N]) -> Self {
        Self::Bytes(value)
    }
}

impl<'a> From<&'a Vec<u8>> for Value<'a> {
    fn from(value: &'a Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl<'a> Value<'a> {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Value::Bytes(v) => v.len(),
            Value::U64(_) => core::mem::size_of::<u64>(),
        }
    }
}

impl<'a, const N: usize> PartialEq<&[u8; N]> for Value<'a> {
    fn eq(&self, rhs: &&[u8; N]) -> bool {
        match self {
            Value::Bytes(v) => v == *rhs,
            Value::U64(_) => false,
        }
    }
}

impl<'a, const N: usize> PartialEq<[u8; N]> for Value<'a> {
    fn eq(&self, rhs: &[u8; N]) -> bool {
        match self {
            Value::Bytes(v) => v == rhs,
            Value::U64(_) => false,
        }
    }
}

impl<'a> PartialEq<[u8]> for Value<'a> {
    fn eq(&self, rhs: &[u8]) -> bool {
        match self {
            Value::Bytes(v) => *v == rhs,
            Value::U64(_) => false,
        }
    }
}

impl<'a> core::fmt::Debug for Value<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match &self {
            Value::Bytes(v) => write!(f, "{:?}", v),
            Value::U64(v) => write!(f, "{}", v),
        }
    }
}

impl core::fmt::Debug for OwnedValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(f, "{:?}", self.as_value())
    }
}

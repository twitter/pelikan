use arrayvec::ArrayVec;
use bytes::BufMut;

use core::fmt;

pub struct OutOfBufferError;

pub trait HeaderValue {
    fn put<B: BufMut>(&self, buf: &mut B) -> Result<(), OutOfBufferError>;

    fn validate(&self) -> bool {
        true
    }

    fn est_len(&self) -> Option<usize> {
        None
    }
}

impl HeaderValue for &'_ [u8] {
    fn put<B: BufMut>(&self, buf: &mut B) -> Result<(), OutOfBufferError> {
        if buf.remaining_mut() < self.len() {
            return Err(OutOfBufferError);
        }

        buf.put_slice(self);

        Ok(())
    }

    fn validate(&self) -> bool {
        crate::util::validate_header_field(self)
    }

    fn est_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl HeaderValue for &'_ str {
    fn put<B: BufMut>(&self, buf: &mut B) -> Result<(), OutOfBufferError> {
        self.as_bytes().put(buf)
    }

    fn validate(&self) -> bool {
        self.as_bytes().validate()
    }

    fn est_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

const fn base10_digits<T>(signed: bool) -> usize {
    (std::mem::size_of::<T>() * 8 + 2) / 3 + (signed as usize)
}

fn reverse_in_place<T>(mut slice: &mut [T]) {
    while slice.len() > 1 {
        let (first, rest) = slice.split_first_mut().unwrap();
        let (last, rest) = rest.split_last_mut().unwrap();
        slice = rest;

        std::mem::swap(first, last);
    }
}

macro_rules! impl_unsigned {
    ( $( $ty:ident ),* ) => {
        $(
            impl HeaderValue for $ty {
                fn put<B: BufMut>(&self, buf: &mut B) -> Result<(), OutOfBufferError> {
                    let mut digits = ArrayVec::<[u8; base10_digits::<$ty>(false)]>::new();
                    let mut value = *self;

                    if value == 0 {
                        digits.push(b'0');
                    }

                    while value != 0 {
                        digits.push((value % 10) as u8 + b'0');
                        value /= 10;
                    }

                    if buf.remaining_mut() < digits.len() {
                        return Err(OutOfBufferError);
                    }

                    reverse_in_place(digits.as_mut_slice());

                    buf.put_slice(digits.as_slice());

                    Ok(())
                }
            }
        )*
    }
}

macro_rules! impl_signed {
    ( $( $ty:ident ),* ) => {
        $(
            impl HeaderValue for $ty {
                fn put<B: BufMut>(&self, buf: &mut B) -> Result<(), OutOfBufferError> {
                    let mut digits = ArrayVec::<[u8; base10_digits::<$ty>(true)]>::new();
                    let mut value = *self;

                    if value == 0 {
                        digits.push(b'0');
                    }

                    if value < 0 {
                        digits.push(b'-');
                        digits.push(10 - value.rem_euclid(10) as u8 + b'0');

                        value = -(value / 10);
                    }

                    while value != 0 {
                        digits.push((value % 10) as u8 + b'0');
                        value /= 10;
                    }

                    if buf.remaining_mut() < digits.len() {
                        return Err(OutOfBufferError);
                    }

                    let start = match digits.get(0) {
                        Some(b'-') => 1,
                        _ => 0,
                    };
                    reverse_in_place(&mut digits.as_mut_slice()[start..]);
                    buf.put_slice(digits.as_slice());

                    Ok(())
                }
            }
        )*
    }
}

impl_unsigned!(usize, u8, u16, u32, u64, u128);
impl_signed!(isize, i8, i16, i32, i64, i128);

pub(crate) struct BufferFmt<'b, B: BufMut>(pub &'b mut B);

impl<'b, B: BufMut> fmt::Write for BufferFmt<'b, B> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.put(self.0).map_err(|_| fmt::Error)
    }
}

#[cfg(feature = "httpdate")]
mod httpdate {
    use super::*;
    use crate::HttpDate;

    use std::fmt::Write;

    impl HeaderValue for HttpDate {
        fn put<B: BufMut>(&self, buf: &mut B) -> Result<(), OutOfBufferError> {
            write!(&mut BufferFmt(buf), "{}", self).map_err(|_| OutOfBufferError)
        }

        fn est_len(&self) -> Option<usize> {
            Some(30)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn escaped(bytes: &[u8]) -> Cow<str> {
        if bytes.iter().copied().all(|c| c.is_ascii()) {
            Cow::Borrowed(std::str::from_utf8(bytes).unwrap())
        } else {
            let string = bytes
                .iter()
                .copied()
                .flat_map(std::ascii::escape_default)
                .map(|c| c as char)
                .collect();

            Cow::Owned(string)
        }
    }

    macro_rules! test_put {
        ($value:expr) => {
            let res = std::panic::catch_unwind(|| {
                let value = $value;
                let expected = format!("{}", value);
                let mut buf = Vec::new();
                if let Err(_) = value.put(&mut buf) {
                    panic!("Not enough buffer in test for {}", stringify!($value));
                }

                assert_eq!(
                    &escaped(&buf),
                    &*expected,
                    "test for {} failed",
                    stringify!($value)
                );
            });

            if let Err(e) = res {
                eprintln!("While running test for {}", stringify!($value));
                std::panic::resume_unwind(e);
            }
        };
    }

    macro_rules! standard_tests {
        ($( $ty:ident ),*) => {
            $(
                test_put!(std::$ty::MAX);
                test_put!(std::$ty::MIN);
                test_put!(0 as $ty);
            )*
        }
    }

    #[test]
    fn serialize_tests() {
        standard_tests!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn test_base10_digits() {
        assert!(base10_digits::<u8>(false) >= 3);
        assert!(base10_digits::<i8>(true) >= 4);
    }
}

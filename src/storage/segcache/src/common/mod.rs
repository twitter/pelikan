// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// TODO(bmartin): consider making this a newtype so that we're able to enforce
// how ThinOption is used through the type system. Currently, we can still do
// numeric comparisons.

// A super thin option type that can be used with reduced-range integers. For
// instance, we can treat signed types < 0 as a None variant. This could also
// be used to wrap unsigned types by reducing the representable range by one bit
pub trait ThinOption: Sized {
    fn is_some(&self) -> bool;
    fn is_none(&self) -> bool;
    fn as_option(&self) -> Option<Self>;
}

// We're currently only using i32
impl ThinOption for i32 {
    fn is_some(&self) -> bool {
        *self >= 0
    }

    fn is_none(&self) -> bool {
        *self < 0
    }

    fn as_option(&self) -> Option<Self> {
        if self.is_some() {
            Some(*self)
        } else {
            None
        }
    }
}

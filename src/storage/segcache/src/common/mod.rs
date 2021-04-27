// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// item info masks and shifts
pub const TAG_MASK: u64 = 0xFFF0_0000_0000_0000;
pub const FREQ_MASK: u64 = 0x000F_F000_0000_0000;
pub const SEG_ID_MASK: u64 = 0x0000_0FFF_FFF0_0000;
pub const OFFSET_MASK: u64 = 0x0000_0000_000F_FFFF;

pub const FREQ_BIT_SHIFT: u64 = 44;
pub const SEG_ID_BIT_SHIFT: u64 = 20;
pub const OFFSET_UNIT_IN_BIT: u64 = 3;

// consts for frequency
pub const CLEAR_FREQ_SMOOTH_MASK: u64 = 0xFFF7_FFFF_FFFF_FFFF;

// bucket info masks and shifts
pub const BUCKET_CHAIN_LEN_MASK: u64 = 0x00FF_0000_0000_0000;
pub const TS_MASK: u64 = 0x0000_FFFF_0000_0000;
pub const CAS_MASK: u64 = 0x0000_0000_FFFF_FFFF;

pub const BUCKET_CHAIN_LEN_BIT_SHIFT: u64 = 48;
pub const TS_BIT_SHIFT: u64 = 32;

// only use the lower 16-bits of the timestamp
pub const PROC_TS_MASK: u64 = 0x0000_0000_0000_FFFF;

// segment constants
pub const SEG_MAGIC: u64 = 0xBADC0FFEEBADCAFE;

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

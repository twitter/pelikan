// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A header which is stored with the item data which contains information about
//! the item representation within the segment.
//!
//! Item Header:
//! ```text
//! ┌──────────────────────────────┬──────────────────────┬──────┬──────┐
//! │      MAGIC (Optional)        │         VLEN         │ KLEN │FLAGS │
//! │                              │                      │      │      │
//! │            32 bit            │        24 bit        │8 bit │ 8bit │
//! │          0xDECAFBAD          │                      │      │      │
//! │0                           31│32                  55│56  63│64  71│
//! └──────────────────────────────┴──────────────────────┴──────┴──────┘
//! ```
//!
//! Flags:
//! ```text
//! ┌──────────────┬──────────────┬──────────────────────────────┐
//! │    TYPED?    │   PADDING    │             OLEN             │
//! │              │              │                              │
//! │    1 bit     │    1 bit     │            6 bit             │
//! │              │              │                              │
//! │      64      │      65      │  66                      71  │
//! └──────────────┴──────────────┴──────────────────────────────┘
//! ```

// item constants

/// The size of the item header in bytes
pub const ITEM_HDR_SIZE: usize = std::mem::size_of::<crate::item::ItemHeader>();

#[cfg(feature = "magic")]
/// The magic bytes to store at the start of the item
pub const ITEM_MAGIC: u32 = 0xDECAFBAD;
#[cfg(feature = "magic")]
/// The length of the item magic in bytes
pub const ITEM_MAGIC_SIZE: usize = std::mem::size_of::<u32>();
#[cfg(not(feature = "magic"))]
#[allow(dead_code)]
/// The length of the item magic in bytes
pub const ITEM_MAGIC_SIZE: usize = 0;

// masks and shifts
// klen/vlen pack together
/// A mask to get the key length from the item header's length field
const KLEN_MASK: u32 = 0x000000FF;
/// A mask used to get the bits containing the item value length from the item
/// header's length field
const VLEN_MASK: u32 = 0xFFFFFF00;
/// The number of bits to shift the length field masked with the value length
/// mask to get the actual value length
const VLEN_SHIFT: u32 = 8;

/// The number of bits to shift the length field masked with the value length
/// mask to get the value type. This is only valid if typed bit is set!!!
const TYPE_MASK: u32 = 0xFF000000;
const TYPE_SHIFT: u32 = 24;

// olen/del/typed
/// A mask to get the optional data length in bytes from the item header's flags
/// field
const OLEN_MASK: u8 = 0b00111111;
/// A mask to get the bit indicating the item value should be treated as a
/// typed value from the item header's flags field
const TYPED_MASK: u8 = 0b10000000;

use core::convert::TryFrom;

#[derive(Copy, Clone, Debug)]
pub(super) enum ValueType {
    U64,
}

impl ValueType {
    pub fn len(&self) -> u32 {
        (match self {
            Self::U64 => std::mem::size_of::<u64>(),
        }) as u32
    }
}

impl TryFrom<u8> for ValueType {
    type Error = ();
    fn try_from(other: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match other {
            0 => Ok(Self::U64),
            _ => Err(()),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<u8> for ValueType {
    fn into(self) -> u8 {
        match self {
            Self::U64 => 0,
        }
    }
}

/// A per-item header which is stored with the item data within a segment. This
/// contains information about the item's raw representation within the segment.
// NOTE: repr(packed) is necessary to get the smallest representation. The
// struct is always taken from an aligned pointer cast. This can potentially
// result in UB when fields are referenced. Fields that require access by
// reference must be strategically placed to ensure alignment and avoid UB.
#[repr(packed)]
pub struct ItemHeader {
    #[cfg(feature = "magic")]
    magic: u32,
    len: u32,  // packs vlen:24 klen:8
    flags: u8, // packs is_num:1, deleted:1, olen:6
}

impl ItemHeader {
    /// Get the magic bytes from the header
    #[cfg(feature = "magic")]
    #[inline]
    pub fn magic(&self) -> u32 {
        self.magic
    }

    /// Write the magic bytes into the header
    #[cfg(feature = "magic")]
    #[inline]
    pub fn set_magic(&mut self) {
        self.magic = ITEM_MAGIC;
    }

    /// Check the magic bytes
    #[inline]
    pub fn check_magic(&self) {
        #[cfg(feature = "magic")]
        assert_eq!(self.magic(), ITEM_MAGIC);
    }

    /// Get the item's key length
    #[inline]
    pub fn klen(&self) -> u8 {
        self.len as u8
    }

    /// Get the item's value length
    #[inline]
    pub fn vlen(&self) -> u32 {
        if self.is_typed() {
            (self.len & !TYPE_MASK) >> VLEN_SHIFT
        } else {
            self.len >> VLEN_SHIFT
        }
    }

    /// get the optional data length
    #[inline]
    pub fn olen(&self) -> u8 {
        self.flags & OLEN_MASK
    }

    /// Is the item a typed value?
    #[inline]
    fn is_typed(&self) -> bool {
        self.flags & TYPED_MASK != 0
    }

    pub(super) fn value_type(&self) -> Option<ValueType> {
        if self.is_typed() {
            if let Ok(t) = ValueType::try_from((self.len >> TYPE_SHIFT) as u8) {
                Some(t)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(super) fn set_type(&mut self, value_type: Option<ValueType>) {
        if let Some(value_type) = value_type {
            self.set_typed(true);
            self.len &= KLEN_MASK;
            self.len |= (value_type.len() as u32) << VLEN_SHIFT;
            let value_type: u8 = value_type.into();
            self.len |= (value_type as u32) << TYPE_SHIFT;
        }
    }

    /// Set the key length by changing just the low byte
    #[inline]
    pub fn set_klen(&mut self, len: u8) {
        self.len = (self.len & !KLEN_MASK) | (len as u32);
    }

    /// Set the value length by changing just the upper bytes
    // TODO(bmartin): where should we do error handling for out-of-range?
    #[inline]
    pub fn set_vlen(&mut self, len: u32) {
        if !self.is_typed() {
            debug_assert!(len <= (u32::MAX >> VLEN_SHIFT));
            self.len = (self.len & !VLEN_MASK) | (len << VLEN_SHIFT);
        }
    }

    /// Mark the item as numeric
    #[inline]
    fn set_typed(&mut self, typed: bool) {
        if typed {
            self.flags |= TYPED_MASK;
        } else {
            self.flags &= !TYPED_MASK;
        }
    }

    pub fn init(&mut self) {
        #[cfg(feature = "magic")]
        self.set_magic();

        self.len = 0;
        self.flags = 0;
    }

    /// Set the optional length
    #[inline]
    pub fn set_olen(&mut self, len: u8) {
        debug_assert!(len <= OLEN_MASK);
        self.flags = (self.flags & !OLEN_MASK) | len;
    }
}

#[cfg(not(feature = "magic"))]
impl std::fmt::Debug for ItemHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("ItemHeader")
            .field("klen", &self.klen())
            .field("vlen", &self.vlen())
            .field("type", &self.value_type())
            .field("olen", &self.olen())
            .finish()
    }
}

#[cfg(feature = "magic")]
impl std::fmt::Debug for ItemHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let magic = self.magic;
        f.debug_struct("ItemHeader")
            .field("magic", &format!("0x{:X}", magic))
            .field("klen", &self.klen())
            .field("vlen", &self.vlen())
            .field("typed", &self.is_typed())
            .field("olen", &self.olen())
            .finish()
    }
}

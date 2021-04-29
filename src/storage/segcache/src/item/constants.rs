// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// item constants
pub const ITEM_HDR_SIZE: usize = std::mem::size_of::<crate::item::ItemHeader>();
#[cfg(feature = "magic")]
pub const ITEM_MAGIC: u32 = 0xDECAFBAD;
#[cfg(feature = "magic")]
pub const ITEM_MAGIC_SIZE: usize = std::mem::size_of::<u32>();

// masks and shifts
// klen/vlen pack together
pub const KLEN_MASK: u32 = 0x000000FF;
pub const VLEN_MASK: u32 = 0xFFFFFF00;

pub const VLEN_SHIFT: u32 = 8;

// olen/del/num
pub const OLEN_MASK: u8 = 0b00111111;
pub const DEL_MASK: u8 = 0b01000000;
pub const NUM_MASK: u8 = 0b10000000;

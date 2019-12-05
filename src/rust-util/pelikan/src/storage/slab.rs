use pelikan_sys::storage::slab::{
    item_rstatus_e, ITEM_ENAN, ITEM_ENOMEM, ITEM_EOTHER, ITEM_EOVERSIZED,
};

use std::fmt;

#[repr(u32)]
#[derive(Debug)]
pub enum ItemError {
    Oversized = ITEM_EOVERSIZED,
    NoMem = ITEM_ENOMEM,
    IsNan = ITEM_ENAN,
    Other = ITEM_EOTHER,
}

impl From<item_rstatus_e> for ItemError {
    fn from(e: item_rstatus_e) -> Self {
        match e {
            ITEM_EOVERSIZED => Self::Oversized,
            ITEM_ENOMEM => Self::NoMem,
            ITEM_ENAN => Self::IsNan,
            _ => Self::Other,
        }
    }
}

impl fmt::Display for ItemError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            Self::Oversized => "EOVERSIZED",
            Self::NoMem => "ENOMEM",
            Self::IsNan => "ENAN",
            Self::Other => "EOTHER",
        };

        fmt.write_str(msg)
    }
}

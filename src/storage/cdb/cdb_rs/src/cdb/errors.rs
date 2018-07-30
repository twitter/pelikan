
// for docs on the 'failure' crate see https://boats.gitlab.io/failure/intro.html

use std::ops::Range;

#[derive(Debug, Fail)]
pub enum CDBError {
    #[fail(display = "Value too large, max_size: {}, val_size: {}", max_size, val_size)]
    ValueTooLarge{max_size: usize, val_size: usize},

    #[fail(
        display = "pointer {:?} out of valid range {:?} for data segment",
        ptr_val, valid_range
    )]
    IndexOutOfDataSegment{valid_range: Range<usize>, ptr_val: usize}
}

impl CDBError {
    pub fn value_too_large(max_size: usize, val_size: usize) -> CDBError {
        CDBError::ValueTooLarge{max_size, val_size}
    }
}

// Copyright (C) 2018-2020 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// for docs on the 'failure' crate see https://boats.gitlab.io/failure/intro.html

use std::ops::Range;

#[derive(Debug, Fail)]
pub enum CDBError {
    #[fail(
        display = "Value too large, max_size: {}, val_size: {}",
        max_size, val_size
    )]
    ValueTooLarge { max_size: usize, val_size: usize },

    #[fail(
        display = "pointer {:?} out of valid range {:?} for data segment",
        ptr_val, valid_range
    )]
    IndexOutOfDataSegment {
        valid_range: Range<usize>,
        ptr_val: usize,
    },
}

impl CDBError {
    pub fn value_too_large(max_size: usize, val_size: usize) -> CDBError {
        CDBError::ValueTooLarge { max_size, val_size }
    }
}

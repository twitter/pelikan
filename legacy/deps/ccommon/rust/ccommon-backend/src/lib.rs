// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
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

//! Rust reimplementations of parts of ccommon.
//!
//! This is a separate library as it is intended to be used as a dependency
//! on C code within ccommon.

pub mod option {
    mod default;
    mod parse;
    mod print;

    pub use self::default::{option_default, option_load_default, OutOfMemoryError};
    pub use self::parse::{option_load, option_set, ParseError, ParseErrorKind};
    pub use self::print::{option_describe, option_describe_all, option_print, option_print_all};
}

pub mod compat;

#[cfg(feature = "c-export")]
mod c_export;

/// Rexports of the functions defined in this crate that correspond
/// to the C functions used in ccommon.
#[cfg(feature = "c-export")]
pub mod c {
    pub use crate::c_export::*;
}

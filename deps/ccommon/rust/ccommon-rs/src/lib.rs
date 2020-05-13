// ccommon - a cache common library.
// Copyright (C) 2018 Twitter, Inc.
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

// Needed to allow derive macros within this crate
extern crate self as ccommon_rs;

#[cfg(feature = "derive")]
pub use ccommon_derive::{Metrics, Options};

pub mod bstring;
pub mod buf;
pub mod log;
pub mod metric;
pub mod option;

mod ccbox;
mod error;

pub use self::ccbox::CCBox;
pub use self::error::{AllocationError, Error};

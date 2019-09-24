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

//! Pelikan is a collection of modules that can be used to
//! build high-performance low-latency servers.
//!
//! This library is an in-progress wrapper of all the main
//! pelikan modules. In the future it may be split apart
//! into a separate crate for each module instead of
//! monolithic `pelikan-sys` and `pelikan` crates.
//!
//! # General Layout
//! The [`core`](crate::core) module contains the event loop
//! and server connection parts of pelikan. Every pelikan-based
//! server will need to use core as the part of the server.
//!
//!

#[macro_use]
extern crate log;

pub mod core;
pub mod protocol;

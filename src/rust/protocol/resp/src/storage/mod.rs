// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub trait Storage {
    fn get(&mut self, request: &Get) -> Response;
    fn set(&mut self, request: &Set) -> Response;
}

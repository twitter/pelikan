// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub trait ExtendFromSlice<T> {
    fn extend(&mut self, src: &[T]);
}

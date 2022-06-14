// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub trait ExtendFromSlice<T> {
    fn extend(&mut self, src: &[T]);
}

pub trait TlsConfig {
    fn certificate_chain(&self) -> Option<String>;

    fn private_key(&self) -> Option<String>;

    fn certificate(&self) -> Option<String>;

    fn ca_file(&self) -> Option<String>;
}

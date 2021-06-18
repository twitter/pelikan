// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Worker threads which are used in multi or single worker mode to handle
//! sending and receiving data on established client sessions

mod multi;
mod single;
mod storage;

pub use self::storage::StorageWorker;
use mio::Token;
pub use multi::MultiWorker;
pub use single::SingleWorker;

use super::EventLoop;

pub struct TokenWrapper<T> {
    inner: T,
    token: Token,
}

impl<T> TokenWrapper<T> {
    pub fn new(inner: T, token: Token) -> Self {
        Self { inner, token }
    }

    pub fn token(&self) -> Token {
        self.token
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

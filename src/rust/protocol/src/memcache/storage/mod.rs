// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::data::MemcacheResponse;

pub trait MemcacheStorage {
    fn get(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse;
    fn gets(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse;
    fn set(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;
    fn add(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;
    fn replace(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;
    fn delete(&mut self, key: &[u8], noreply: bool) -> MemcacheResponse;
    fn cas(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
        cas: u64,
    ) -> MemcacheResponse;
}

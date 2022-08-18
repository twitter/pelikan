// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate log;

pub use bytes::buf::UninitSlice;
pub use bytes::{Buf, BufMut};

use core::borrow::{Borrow, BorrowMut};
use std::alloc::*;

/// A simple growable byte buffer, represented as a contiguous range of bytes
pub struct Buffer {
    ptr: *mut u8,
    cap: usize,
    read_offset: usize,
    write_offset: usize,
    target_size: usize,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

impl Buffer {
    pub fn new(target_size: usize) -> Self {
        let layout = Layout::array::<u8>(target_size).unwrap();
        let ptr = unsafe { alloc(layout) };
        let cap = target_size;
        let read_offset = 0;
        let write_offset = 0;

        Self {
            ptr,
            cap,
            read_offset,
            write_offset,
            target_size,
        }
    }

    pub fn reserve(&mut self, amt: usize) {
        // if the buffer is empty, reset the offsets
        if self.remaining() == 0 {
            self.read_offset = 0;
            self.write_offset = 0;
        }

        // grow the buffer if needed, uses a multiple of the target size
        if amt > self.remaining_mut() {
            info!("growing buffer");
            // round the amount up to a multiple of the target size
            let amt = ((amt / self.target_size) as usize + 1) * self.target_size;

            // new size will be the current capacity plus the amount needed
            let size = self.cap + amt;
            let layout = Layout::array::<u8>(self.cap).unwrap();
            self.ptr = unsafe { realloc(self.ptr, layout, size) };
            self.cap = size;
        }
    }

    pub fn clear(&mut self) {
        self.read_offset = 0;
        self.write_offset = 0;

        // if the buffer is oversized, shrink to the target size
        if self.cap > self.target_size {
            info!("shrinking buffer");
            let layout = Layout::array::<u8>(self.cap).unwrap();
            self.ptr = unsafe { realloc(self.ptr, layout, self.target_size) };
            self.cap = self.target_size;
        }
    }

    pub fn compact(&mut self) {
        // if the buffer is empty, we clear the buffer and return
        if self.remaining() == 0 {
            self.clear();
            return;
        }

        // if the buffer data is deep into the buffer, we can copy the data to
        // the start of the buffer to make additional space available for writes
        if self.read_offset > self.target_size {
            println!("compacting");
            if self.remaining() < self.read_offset {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.ptr.add(self.read_offset),
                        self.ptr,
                        self.remaining(),
                    );
                }
            } else {
                unsafe {
                    std::ptr::copy(self.ptr.add(self.read_offset), self.ptr, self.remaining());
                }
            }
            self.read_offset = 0;
            self.write_offset = self.remaining();
        }

        // if the buffer is large and contents fit within the target size,
        // shrink the buffer down to the target size
        if self.cap > self.target_size && self.remaining() < self.target_size {
            let layout = Layout::array::<u8>(self.cap).unwrap();
            self.ptr = unsafe { realloc(self.ptr, layout, self.cap + self.target_size) };
        }
    }

    // get the current write position as a pointer
    // remaining_mut should be used as the length
    pub fn write_ptr(&mut self) -> *mut u8 {
        unsafe { self.ptr.add(self.write_offset) }
    }

    // get the current read position as a pointer
    // remaining should be used as the length
    pub fn read_ptr(&mut self) -> *mut u8 {
        unsafe { self.ptr.add(self.read_offset) }
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.add(self.read_offset), self.remaining()) }
    }
}

impl BorrowMut<[u8]> for Buffer {
    fn borrow_mut(self: &mut Buffer) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr.add(self.write_offset), self.remaining_mut())
        }
    }
}

impl Buf for Buffer {
    fn remaining(&self) -> usize {
        self.write_offset - self.read_offset
    }

    fn chunk(&self) -> &[u8] {
        self.borrow()
    }

    fn advance(&mut self, amt: usize) {
        self.read_offset = std::cmp::min(self.read_offset + amt, self.write_offset);
        self.compact();
    }
}

unsafe impl BufMut for Buffer {
    fn remaining_mut(&self) -> usize {
        self.cap - self.write_offset
    }

    unsafe fn advance_mut(&mut self, amt: usize) {
        self.write_offset = std::cmp::min(self.write_offset + amt, self.cap);
    }

    fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
        unsafe {
            UninitSlice::from_raw_parts_mut(self.ptr.add(self.write_offset), self.remaining_mut())
        }
    }

    fn put<T: Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        while src.has_remaining() {
            let chunk = src.chunk();
            let len = chunk.len();
            self.put_slice(chunk);
            src.advance(len);
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.reserve(src.len());
        assert!(self.remaining_mut() >= src.len());
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.ptr.add(self.write_offset), src.len());
        }
        unsafe {
            self.advance_mut(src.len());
        }
    }
}

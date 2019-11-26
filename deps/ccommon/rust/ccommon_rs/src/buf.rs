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

use cc_binding::buf;

use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

use bytes::{Buf as BytesBuf, BufMut as BytesBufMut};

/// A non-owned buffer, can be read from and written to.
///
/// # Safety
/// This is a self-referential struct that assumes it is
/// followed by it's data. Attempting to move it or create
/// a new instance (via transmute) will cause UB.
#[repr(transparent)]
#[derive(Debug)]
pub struct Buf {
    buf: buf,
}

impl Buf {
    pub unsafe fn from_ptr<'a>(buf: *const buf) -> &'a Buf {
        &*(buf as *const Buf)
    }

    pub unsafe fn from_mut_ptr<'a>(buf: *mut buf) -> &'a mut Buf {
        &mut *(buf as *mut Buf)
    }

    pub fn as_ptr(&self) -> *const buf {
        &self.buf as *const buf
    }

    pub fn as_mut_ptr(&mut self) -> *mut buf {
        &mut self.buf as *mut buf
    }

    #[deprecated(note = "use from_ptr instead")]
    pub unsafe fn from_raw<'a>(buf: *const buf) -> &'a Buf {
        &*(buf as *const Buf)
    }

    #[deprecated(note = "use from_mut_ptr instead")]
    pub unsafe fn from_raw_mut<'a>(buf: *mut buf) -> &'a mut Buf {
        &mut *(buf as *mut Buf)
    }

    #[deprecated(note = "use as_ptr instead")]
    #[allow(clippy::wrong_self_convention)]
    pub fn into_raw(&self) -> *const buf {
        &self.buf as *const _
    }

    #[deprecated(note = "use as_mut_ptr instead")]
    #[allow(clippy::wrong_self_convention)]
    pub fn into_raw_mut(&mut self) -> *mut buf {
        &mut self.buf as *mut _
    }

    /// The number of bytes that can still be written to the
    /// buffer before it is full.
    pub fn write_size(&self) -> usize {
        assert!(self.buf.wpos as usize <= self.buf.end as usize);

        self.buf.end as usize - self.buf.wpos as usize
    }

    /// The number of bytes left to read from the buffer
    pub fn read_size(&self) -> usize {
        assert!(self.buf.rpos as usize <= self.buf.wpos as usize);

        self.buf.wpos as usize - self.buf.rpos as usize
    }

    pub fn capacity(&self) -> usize {
        assert!(unsafe { self.buf.begin.as_ptr() } as usize <= self.buf.end as usize);

        self.buf.end as usize - unsafe { self.buf.begin.as_ptr() } as usize
    }

    /// Additional capacity required to write count bytes to the buffer
    pub fn new_cap(&self, bytes: usize) -> usize {
        assert!(unsafe { self.buf.begin.as_ptr() } as usize <= self.buf.wpos as usize);

        bytes.saturating_sub(self.write_size())
    }

    /// Clear the buffer and remove it from any allocation pool
    pub fn reset(&mut self) {
        self.buf.next.stqe_next = std::ptr::null_mut();
        self.buf.free = false;
        self.buf.rpos = unsafe { self.buf.begin.as_mut_ptr() };
        self.buf.wpos = self.buf.rpos;
    }

    /// Shift data in the buffer to the end of the buffer.
    pub fn rshift(&mut self) {
        let size = self.read_size();

        if size > 0 {
            unsafe {
                std::ptr::copy(self.buf.rpos, self.buf.rpos.offset(-(size as isize)), size);
            }
        }

        self.buf.rpos = unsafe { self.buf.end.offset(-(size as isize)) };
        self.buf.wpos = self.buf.end;
    }

    /// Shift data in the buffer to the start of the buffer.
    pub fn lshift(&mut self) {
        let size = self.read_size();

        if size > 0 {
            unsafe {
                std::ptr::copy(self.buf.rpos, self.buf.begin.as_mut_ptr(), size);
            }
        }

        self.buf.rpos = unsafe { self.buf.begin.as_mut_ptr() };
        self.buf.wpos = self.buf.rpos.wrapping_add(size);
    }

    /// Get the currently valid part of the buffer as a slice
    pub fn as_slice(&self) -> &[u8] {
        use std::slice;

        unsafe { slice::from_raw_parts(self.buf.rpos as *const u8, self.read_size()) }
    }

    /// Get the currently valid part of the buffer as a slice
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        use std::slice;

        unsafe { slice::from_raw_parts_mut(self.buf.rpos as *mut u8, self.read_size()) }
    }
}

/// An owned buffer.
///
/// In addition to all the operations supported by `Buf`
/// an `OwnedBuf` also supports some resizing operations.
#[repr(transparent)]
#[derive(Debug)]
pub struct OwnedBuf {
    buf: *mut buf,
}

impl OwnedBuf {
    pub unsafe fn from_raw(buf: *mut buf) -> Self {
        Self { buf }
    }

    pub fn into_raw(self) -> *mut buf {
        let buf = self.buf;
        // Don't want to run drop on the destructor
        std::mem::forget(self);
        buf
    }

    pub fn as_ptr(&self) -> *const buf {
        self.buf
    }

    pub fn as_mut_ptr(&mut self) -> *mut buf {
        self.buf
    }

    /// Double the size of the buffer.
    pub fn double(&mut self) -> Result<(), crate::Error> {
        use cc_binding::dbuf_double;

        unsafe {
            let status = dbuf_double(&mut self.buf as *mut _);

            if status != 0 {
                Err(status.into())
            } else {
                Ok(())
            }
        }
    }

    /// Shrink the buffer to the max of the initial size or the content size.
    pub fn shrink(&mut self) -> Result<(), crate::Error> {
        use cc_binding::dbuf_shrink;

        unsafe {
            let status = dbuf_shrink(&mut self.buf as *mut _);

            if status != 0 {
                Err(status.into())
            } else {
                Ok(())
            }
        }
    }

    /// Resize the buffer to fit the required capacity.
    ///
    /// # Panics
    /// Panics if `cap` is greater than `u32::MAX`.
    pub fn fit(&mut self, cap: usize) -> Result<(), crate::Error> {
        use cc_binding::dbuf_fit;

        assert!((cap as u64) <= std::u32::MAX as u64);

        unsafe {
            let status = dbuf_fit(&mut self.buf as *mut _, cap as u32);

            if status != 0 {
                Err(status.into())
            } else {
                Ok(())
            }
        }
    }
}

unsafe impl Send for OwnedBuf {}
unsafe impl Sync for OwnedBuf {}

impl Drop for OwnedBuf {
    fn drop(&mut self) {
        use cc_binding::buf_destroy;

        unsafe {
            buf_destroy(&mut self.buf as *mut _);
        }
    }
}

impl Deref for OwnedBuf {
    type Target = Buf;

    fn deref(&self) -> &Buf {
        unsafe { Buf::from_ptr(self.buf) }
    }
}

impl DerefMut for OwnedBuf {
    fn deref_mut(&mut self) -> &mut Buf {
        unsafe { Buf::from_mut_ptr(self.buf) }
    }
}

impl Read for Buf {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let len = self.read_size().min(out.len());

        unsafe { std::ptr::copy_nonoverlapping(self.buf.rpos as *const u8, out.as_mut_ptr(), len) }
        self.buf.rpos = self.buf.rpos.wrapping_add(len);

        Ok(len)
    }
}

impl Write for Buf {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        if src.is_empty() {
            return Ok(0);
        }

        let len = self.write_size().min(src.len());

        unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), self.buf.wpos as *mut _, src.len()) }
        self.buf.wpos = self.buf.wpos.wrapping_add(len);

        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for OwnedBuf {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }
}

impl Write for OwnedBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }
}

impl BytesBuf for Buf {
    fn remaining(&self) -> usize {
        self.read_size()
    }

    fn bytes(&self) -> &[u8] {
        self.as_slice()
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.read_size());
        self.buf.rpos = self.buf.rpos.wrapping_add(cnt);
    }
}

impl BytesBufMut for Buf {
    fn remaining_mut(&self) -> usize {
        self.write_size()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        assert!(cnt <= self.write_size());
        self.buf.wpos = self.buf.wpos.wrapping_add(cnt);
    }

    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe {
            std::slice::from_raw_parts_mut(self.buf.wpos as *mut MaybeUninit<u8>, self.write_size())
        }
    }
}

impl BytesBuf for OwnedBuf {
    fn remaining(&self) -> usize {
        (**self).remaining()
    }

    fn bytes(&self) -> &[u8] {
        BytesBuf::bytes(&**self)
    }

    fn advance(&mut self, cnt: usize) {
        (**self).advance(cnt)
    }
}

impl BytesBufMut for OwnedBuf {
    fn remaining_mut(&self) -> usize {
        (**self).remaining_mut()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        (**self).advance_mut(cnt)
    }

    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        BytesBufMut::bytes_mut(&mut **self)
    }
}

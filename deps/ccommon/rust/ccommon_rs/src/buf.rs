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

use cc_binding::buf;

#[repr(transparent)]
pub struct Buf {
    buf: buf
}

impl Buf {
    pub unsafe fn from_raw<'a>(buf: *const buf) -> &'a Buf {
        &*(buf as *const Buf)
    }

    pub unsafe fn from_raw_mut<'a>(buf: *mut buf) -> &'a mut Buf {
        &mut *(buf as *mut Buf)
    }

    pub fn into_raw(&self) -> *const buf {
        &self.buf as *const _
    }
    pub fn into_raw_mut(&mut self) -> *mut buf {
        &mut self.buf as *mut _
    }

    pub fn write_size(&self) -> usize {
        assert!(self.buf.wpos as usize <= self.buf.end as usize);

        self.buf.end as usize - self.buf.wpos as usize
    }
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

        return bytes.saturating_sub(self.write_size());
    }

    pub fn reset(&mut self) {
        self.buf.next.stqe_next = std::ptr::null_mut();
        self.buf.free = false;
        self.buf.rpos = unsafe { self.buf.begin.as_mut_ptr() };
        self.buf.wpos = self.buf.rpos;
    }

    pub fn read(&mut self, out: &mut [u8]) -> usize {
        let len = self.read_size().min(out.len());

        unsafe {
            std::ptr::copy_nonoverlapping(self.buf.rpos as *const u8, out.as_mut_ptr(), len)
        }
        self.buf.rpos = self.buf.rpos.wrapping_add(len);

        len
    }

    pub fn write(&mut self, src: &[u8]) -> usize {
        if src.is_empty() {
            return 0;
        }

        let len = self.write_size().min(src.len());

        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.buf.wpos as *mut _, src.len())
        }
        self.buf.wpos = self.buf.wpos.wrapping_add(len);

        len
    }

    pub fn rshift(&mut self) {
        let size = self.read_size();

        if size > 0 {
            unsafe {
                std::ptr::copy(
                    self.buf.rpos,
                    self.buf.rpos.offset(-(size as isize)),
                    size
                );
            }
        }

        self.buf.rpos = unsafe { self.buf.end.offset(-(size as isize)) };
        self.buf.wpos = self.buf.end;
    }

    pub fn lshift(&mut self) {
        let size = self.read_size();

        if size > 0 {
            unsafe {
                std::ptr::copy(
                    self.buf.rpos,
                    self.buf.begin.as_mut_ptr(),
                    size
                );
            }
        }

        self.buf.rpos = unsafe { self.buf.begin.as_mut_ptr() };
        self.buf.wpos = self.buf.rpos.wrapping_add(size);
    }
}
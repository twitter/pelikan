use std::alloc::*;
use core::ptr::NonNull;


use bytes::Buf;
use bytes::BufMut;
use net::Stream;

use std::io::Read;
use std::io::Write;

use std::io::Result;
use std::io::ErrorKind;

// defines the desired amount of data to read from the underlying stream 
const TARGET_READ_SIZE: usize = 4096;

/// A simple growable byte buffer, represented as a contiguous range of bytes
pub struct Buffer {
    ptr: NonNull<u8>,
    cap: usize,
    read_offset: usize,
    write_offset: usize,
    target_size: usize,
}

impl Buffer {
    pub fn new(target_size: usize) -> Self {
        let layout = Layout::array::<u8>(target_size).unwrap();
        let ptr = unsafe { alloc(layout) };
        let cap = target_size;
        let read_offset = 0;
        let write_offset = 0;

        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            cap,
            read_offset,
            write_offset,
            target_size,
        }
    }
}

// impl Buffer {
//     pub fn new(target_capacity: usize) -> Self {
//         Self {
//             buffer: vec![0; target_capacity],
//             read_offset: 0,
//             write_offset: 0,
//             target_capacity,
//         }
//     }
// }

// impl Buf for Buffer {
//     fn remaining(&self) -> usize {
//         self.write_offset - self.read_offset
//     }

//     fn chunk(&self) -> &[u8] {
//         &self.buffer[self.read_offset..self.write_offset]
//     }

//     fn advance(&mut self, amt: usize) {
//         self.read_offset = std::cmp::min(self.read_offset + amt, self.write_offset);
//     }
// }

// unsafe impl BufMut for Buffer {
//     fn remaining_mut(&self) -> usize {
//         self.buffer.len() - self.write_offset
//     }

//     unsafe fn advance_mut(&mut self, amt: usize) {
//         self.write_offset = std::cmp::min(self.write_offset + amt, self.buffer.len());
//     }

//     fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
//         unsafe {
//             UninitSlice::from_raw_parts_mut(self.buffer.as_mut_ptr().add(self.write_offset), self.remaining_mut())
//         }
//     }
// }

struct Session<B> {
    stream: Stream,
    read_buffer: B,
    write_buffer: B,
}

impl<B> Session<B> where B:  {
    // fn fill(&mut self) -> Result<usize> {
    //     if self.read_buffer.

    //     let mut tmp = [0; TARGET_READ_SIZE];

    // }
}

// impl<B> BufRead for Session<B> where B: Buf + BufMut + BufRead {
//     fn fill_buf(&mut self) -> Result<&[u8]> {

//     }
//     fn consume(&mut self, amt: usize) {
//         self.read_buffer.advance(amt)
//     }
// }

impl<B> Read for Session<B> where B: Buf + BufMut {
    // NOTE: this implementation will make a syscall only if the caller wants
    // more data than is currently in the internal read buffer.
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        // If the read buffer is empty and the provided buffer is sufficiently
        // large, we bypass the read buffer and read directly into the provided
        // buffer.
        if self.read_buffer.remaining() == 0 && buf.len() >= TARGET_READ_SIZE {
            return self.stream.read(buf);
        }

        // TODO(bmartin): consider eliminating the double-copy here. This simple
        // implementation copies from the stream into the read buffer and then
        // to the provided buffer.
        if self.read_buffer.remaining() < buf.len() {
            let mut tmp = [0; TARGET_READ_SIZE];
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    // This means the underlying stream is closed, we need to
                    // notify the caller by returning this result.
                    return Ok(0);
                }
                Ok(n) => {
                    // Successfully read 'n' bytes from the stream into the
                    // temporary buffer. Copy them into the read buffer.
                    self.read_buffer.put_slice(&tmp[0..n]);
                }
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock | ErrorKind::Interrupted => {}
                    _ => {
                        return Err(e);
                    }
                }
            }
        }

        // NOTE: since byte buffers *may* be non-contiguous, we need to loop.

        let mut copied = 0;

        while copied <= buf.len() {
            let chunk = self.read_buffer.chunk();
            if chunk.is_empty() {
                break;
            }
            let len = std::cmp::min(chunk.len(), buf.len() - copied);
            unsafe {
                std::ptr::copy_nonoverlapping(chunk.as_ptr(), buf.as_mut_ptr().add(copied), len);
            }
            copied += len;
        }
        
        self.read_buffer.advance(copied);
        Ok(buf.len())
    }
}

impl<B> Write for Session<B> where B: Buf + BufMut {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        // Bypass the write buffer and write directly to the stream if the write
        // buffer is currently empty. Any bytes that failed to write to the
        // stream are copied into the write buffer.
        if self.write_buffer.remaining() == 0 {
            if let Ok(amt) = self.stream.write(buf) {
                if amt == buf.len() {
                    return Ok(amt);
                } else {
                    self.write_buffer.put_slice(&buf[amt..]);
                    return Ok(buf.len());
                }
            }
        }

        self.write_buffer.put_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        // NOTE: since byte buffers *may* be non-contiguous, we need to loop

        loop {
            let chunk = self.write_buffer.chunk();
            if chunk.is_empty() {
                break;
            }
            let amt = self.stream.write(chunk)?;
            self.write_buffer.advance(amt);
        }

        Ok(())
    }
}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

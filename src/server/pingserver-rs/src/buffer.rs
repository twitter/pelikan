// Copyright 2019 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![deny(clippy::all)]

use bytes::{Buf, BufMut, BytesMut};
use log::*;

use std::borrow::Borrow;
use std::io::{self, Cursor, Error, Read, Write};

#[derive(Debug)]
/// A bi-directional buffer
pub struct Buffer {
    rx: BytesMut,
    tx: BytesMut,
}

impl Buffer {
    /// Creates a new `Buffer`
    pub fn new(rx: usize, tx: usize) -> Buffer {
        Buffer {
            rx: BytesMut::with_capacity(rx),
            tx: BytesMut::with_capacity(tx),
        }
    }

    /// Writes from this `Buffer` to some sink which implements `Write`
    pub fn write_to<T: Write>(&mut self, sink: &mut T) -> io::Result<Option<usize>> {
        let buffer: &[u8] = self.tx.borrow();
        match sink.try_write_buf(&mut Cursor::new(buffer)) {
            Ok(Some(b)) => {
                if b == buffer.len() {
                    trace!("tx: {} mut_remaining", self.tx.remaining_mut());
                    self.tx.advance(b);
                    Ok(Some(b))
                } else {
                    self.tx.advance(b);
                    debug!("connection buffer not flushed completely");
                    Err(io::Error::new(io::ErrorKind::Other, "incomplete"))
                }
            }
            Ok(None) => Err(io::Error::new(io::ErrorKind::Other, "spurious flush")),
            Err(e) => Err(e),
        }
    }

    /// Reads from a source implementing `Read` into this `Buffer`
    pub fn read_from<T: Read>(&mut self, source: &mut T) -> io::Result<Option<usize>> {
        // loop the read operation and grow the buffer as necessary
        let mut bytes = 0;
        loop {
            if let Some(count) = source.try_read_buf(&mut self.rx)? {
                bytes += count;
                if self.rx.has_remaining_mut() {
                    break;
                } else {
                    self.rx.reserve(4096);
                }
            } else {
                return Ok(None);
            }
        }
        Ok(Some(bytes))
    }

    /// Clear the `Buffer`
    pub fn clear(&mut self) {
        self.rx.clear();
        self.tx.clear();
    }

    /// return a reference to the bytes in the rx buffer
    /// useful for zero-copy response handling
    pub fn rx_buffer(&self) -> &[u8] {
        self.rx.borrow()
    }

    pub fn tx_pending(&self) -> bool {
        !self.tx.is_empty()
    }
}

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.tx.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

impl Read for Buffer {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut buffer: Cursor<&[u8]> = Cursor::new(self.rx.borrow());
        buffer.read(buf)
    }
}

pub trait TryRead {
    fn try_read_buf<B: BufMut>(&mut self, buf: &mut B) -> io::Result<Option<usize>>
    where
        Self: Sized,
    {
        // Reads the length of the slice supplied by buf.mut_bytes into the buffer
        // This is not guaranteed to consume an entire datagram or segment.
        // If your protocol is msg based (instead of continuous stream) you should
        // ensure that your buffer is large enough to hold an entire segment
        // (1532 bytes if not jumbo frames)
        let res = self.try_read(unsafe { buf.bytes_mut() });

        if let Ok(Some(cnt)) = res {
            unsafe {
                buf.advance_mut(cnt);
            }
        }

        res
    }

    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;
}

pub trait TryWrite {
    fn try_write_buf<B: Buf>(&mut self, buf: &mut B) -> io::Result<Option<usize>>
    where
        Self: Sized,
    {
        let res = self.try_write(buf.bytes());

        if let Ok(Some(cnt)) = res {
            buf.advance(cnt);
        }

        res
    }

    fn try_write(&mut self, buf: &[u8]) -> io::Result<Option<usize>>;
}

impl<T: Read> TryRead for T {
    fn try_read(&mut self, dst: &mut [u8]) -> io::Result<Option<usize>> {
        self.read(dst).map_non_block()
    }
}

impl<T: Write> TryWrite for T {
    fn try_write(&mut self, src: &[u8]) -> io::Result<Option<usize>> {
        self.write(src).map_non_block()
    }
}

/// A helper trait to provide the `map_non_block` function on Results.
trait MapNonBlock<T> {
    /// Maps a `Result<T>` to a `Result<Option<T>>` by converting
    /// operation-would-block errors into `Ok(None)`.
    fn map_non_block(self) -> io::Result<Option<T>>;
}

impl<T> MapNonBlock<T> for io::Result<T> {
    fn map_non_block(self) -> io::Result<Option<T>> {
        use std::io::ErrorKind::WouldBlock;

        match self {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                if let WouldBlock = err.kind() {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_and_write_to() {
        let mut buffer = Buffer::new(4096, 4096);

        let messages: Vec<&[u8]> = vec![
            b"break the ice",
            b"he hath eaten me out of house and home",
            b"brevity is the soul of wit",
        ];

        for message in messages {
            buffer.write(message).expect("write failed");
            let mut sink = Vec::new();
            if let Ok(Some(len)) = buffer.write_to(&mut sink) {
                assert_eq!(sink.len(), len);
                assert_eq!(sink, message);
            }
        }
    }

    #[test]
    fn read_from_and_read() {
        let mut buffer = Buffer::new(4096, 4096);

        let mut messages: Vec<&[u8]> = vec![
            b"break the ice",
            b"he hath eaten me out of house and home",
            b"brevity is the soul of wit",
        ];

        for message in &mut messages {
            let check = message.clone();
            buffer.read_from(message).expect("read from failed");
            let mut sink = vec![0; 4096];
            if let Ok(len) = buffer.read(&mut sink) {
                sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
                assert_eq!(sink.len(), len); // sink contains the number of bytes read into it
                assert_eq!(sink.len(), check.len()); // sink has same number of bytes as check string
                assert_eq!(sink.as_slice(), check); // sink has same data as check string
                buffer.clear();
            }
        }
    }

    #[test]
    fn read_from_multiple() {
        let mut buffer = Buffer::new(4096, 4096);

        let mut messages: Vec<&[u8]> = vec![b"brave ", b"new ", b"world"];

        for message in &mut messages {
            buffer.read_from(message).expect("read from failed");
        }

        let check = b"brave new world";
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), len); // sink contains the number of bytes read into it
            assert_eq!(sink.len(), check.len()); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), check); // sink has same data as check string
            buffer.clear();
        }
        sink.clear();
        if let Ok(len) = buffer.read(&mut sink) {
            assert_eq!(len, 0);
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), len); // sink contains the number of bytes read into it
            assert_eq!(sink.as_slice(), b""); // sink has same data as check string
            buffer.clear();
        }
    }

    #[test]
    fn write_to_multiple() {
        let mut buffer = Buffer::new(4096, 4096);

        buffer.write(b"DEAD").expect("write failed");

        let check = b"DEAD";

        let mut sink = Vec::new();
        assert_eq!(buffer.write_to(&mut sink).unwrap().unwrap(), 4);
        assert_eq!(sink.len(), check.len());
        assert_eq!(sink.as_slice(), check);

        buffer.write(b"BEEF").expect("write failed");

        let check = b"BEEF";

        let mut sink = Vec::new();
        assert_eq!(buffer.write_to(&mut sink).unwrap().unwrap(), 4);
        assert_eq!(sink.len(), check.len());
        assert_eq!(sink.as_slice(), check);

        buffer.write(b"DEAD").expect("write failed");
        buffer.write(b"BEEF").expect("write failed");

        let check = b"DEADBEEF";

        let mut sink = Vec::new();
        assert_eq!(buffer.write_to(&mut sink).unwrap().unwrap(), 8);
        assert_eq!(sink.len(), check.len());
        assert_eq!(sink.as_slice(), check);
    }

    #[test]
    fn partial_read() {
        let mut buffer = Buffer::new(4096, 4096);

        let mut messages: Vec<&[u8]> = vec![b"DEAD", b"BEEF"];

        buffer
            .read_from(&mut messages[0])
            .expect("read from failed");

        // reading first time should have the contents we just wrote
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), 4); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), b"DEAD"); // sink has same data as check string
        }

        // reading again gives us the same result
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), 4); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), b"DEAD"); // sink has same data as check string
        }

        // append to the buffer and read to get all written data
        buffer
            .read_from(&mut messages[1])
            .expect("read from failed");
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), 8); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), b"DEADBEEF"); // sink has same data as check string
        }
    }
}

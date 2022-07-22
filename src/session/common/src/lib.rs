// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::fmt::Debug;
use protocol_common::Parse;
use protocol_common::Compose;
use core::marker::PhantomData;
use rustcommon_time::Nanoseconds;
use std::collections::VecDeque;
pub use buffer::*;

use protocol_common::ParseError;

use ::net::*;
use core::borrow::{Borrow, BorrowMut};
use std::io::ErrorKind;
use std::io::Read;
use std::io::Result;
use std::io::Write;

type Instant = rustcommon_time::Instant<Nanoseconds<u64>>;

// The size of one kilobyte, in bytes
const KB: usize = 1024;

// If the read buffer has less than this amount available before a read, we will
// grow the read buffer. The selected value is set to the size of a single page.
const BUFFER_MIN_FREE: usize = 4 * KB;

// The target size of the read operations, the selected value is the upper-bound
// on TLS fragment size as per RFC 5246:
// https://datatracker.ietf.org/doc/html/rfc5246#section-6.2.1
const TARGET_READ_SIZE: usize = 16 * KB;

/// A `Session` is an underlying `Stream` with its read and write buffers. This
/// abstraction allows the caller to efficiently read from the underlying stream
/// by buffering the incoming bytes. It also allows for efficient writing by
/// first buffering writes to the underlying stream.
pub struct Session {
    stream: Stream,
    read_buffer: Buffer,
    write_buffer: Buffer,
}

impl Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.stream)
    }
}

impl Session {
    /// Construct a new `Session` from a `Stream` and read and write
    /// `SessionBuffer`s.
    pub fn new(stream: Stream, read_buffer: Buffer, write_buffer: Buffer) -> Self {
        Self {
            stream,
            read_buffer,
            write_buffer,
        }
    }

    /// Return the event `Interest`s for the `Session`.
    pub fn interest(&self) -> Interest {
        if self.write_buffer.has_remaining() {
            self.stream.interest().add(Interest::WRITABLE)
        } else {
            self.stream.interest()
        }
    }

    /// Indicates if the `Session` can be considered established, meaning that
    /// any underlying stream negotation and handshaking is completed.
    pub fn is_established(&self) -> bool {
        self.stream.is_established()
    }

    pub fn is_handshaking(&self) -> bool {
        self.stream.is_handshaking()
    }

    /// Fill the read buffer by calling read on the underlying stream until read
    /// would block. Returns the number of bytes read. `Ok(0)` indicates that
    /// the remote side has closed the stream.
    pub fn fill(&mut self) -> Result<usize> {
        let mut read = 0;

        loop {
            // if the buffer has too little space available, expand it
            if self.read_buffer.remaining_mut() < BUFFER_MIN_FREE {
                self.read_buffer.reserve(TARGET_READ_SIZE);
            }

            // read directly into the read buffer
            match self.stream.read(self.read_buffer.borrow_mut()) {
                Ok(0) => {
                    // This means the underlying stream is closed, we need to
                    // notify the caller by returning this result.
                    return Ok(0);
                }
                Ok(n) => {
                    // Successfully read 'n' bytes from the stream into the
                    // buffer. Advance the write position.
                    unsafe {
                        self.read_buffer.advance_mut(n);
                    }
                    read += n;
                }
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => {
                        if read == 0 {
                            return Err(e);
                        } else {
                            return Ok(read);
                        }
                    }
                    ErrorKind::Interrupted => {}
                    _ => {
                        return Err(e);
                    }
                },
            }
        }
    }

    pub fn consume(&mut self, amt: usize) {
        self.read_buffer.advance(amt)
    }

    pub fn write_pending(&self) -> usize {
        self.write_buffer.remaining()
    }

    pub fn flush(&mut self) -> Result<usize> {
        let mut flushed = 0;
        while self.write_buffer.has_remaining() {
            match self.stream.write(self.write_buffer.borrow()) {
                Ok(amt) => {
                    self.write_buffer.advance(amt);
                    flushed += amt;
                }
                Err(e) => match e.kind() {
                    ErrorKind::Interrupted => {
                        // this should be retried immediately
                    }
                    _ => {
                        // all other errors get bubbled up
                        return Err(e);
                    }
                },
            }
        }

        Ok(flushed)
    }
}

// NOTE: this is opioniated in that we set the buffer sizes, but should be an
// acceptable default for most session construction
impl From<Stream> for Session {
    fn from(other: Stream) -> Self {
        Self::new(
            other,
            Buffer::new(TARGET_READ_SIZE),
            Buffer::new(TARGET_READ_SIZE),
        )
    }
}

// impl Read for Session {
//     // NOTE: this implementation will make a syscall only if the caller wants
//     // more data than is currently in the internal read buffer.
//     fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
//         // If the read buffer is empty and the provided buffer is sufficiently
//         // large, we bypass the read buffer and read directly into the provided
//         // buffer.
//         if self.read_buffer.remaining() == 0 && buf.len() >= self.read_buffer.remaining_mut() {
//             return self.stream.read(buf);
//         }

//         // TODO(bmartin): consider eliminating the double-copy here. This simple
//         // implementation copies from the stream into the read buffer and then
//         // to the provided buffer.
//         if self.read_buffer.remaining() < buf.len() {
//             self.read_buffer.put_slice(buf);
//             match self.stream.read(self.read_buffer.borrow_mut()) {
//                 Ok(0) => {
//                     // This means the underlying stream is closed, we need to
//                     // notify the caller by returning this result.
//                     return Ok(0);
//                 }
//                 Ok(n) => {
//                     // Successfully read 'n' bytes from the stream into the
//                     // buffer. Advance the write position.
//                     unsafe {
//                         self.read_buffer.advance_mut(n);
//                     }
//                 }
//                 Err(e) => match e.kind() {
//                     ErrorKind::WouldBlock | ErrorKind::Interrupted => {}
//                     _ => {
//                         return Err(e);
//                     }
//                 },
//             }
//         }

//         let len = std::cmp::min(self.read_buffer.remaining(), buf.len());
//         let src: &[u8] = self.read_buffer.borrow();
//         unsafe {
//             std::ptr::copy_nonoverlapping(src.as_ptr(), buf.as_mut_ptr(), len);
//         }
//         self.read_buffer.advance(len);

//         Ok(buf.len())
//     }
// }

// impl Write for Session {
//     fn write(&mut self, buf: &[u8]) -> Result<usize> {
//         // if the contents fit in the write buffer, copy them and return
//         if buf.len() <= self.write_buffer.remaining_mut() {
//             self.write_buffer.put_slice(buf);
//             return Ok(buf.len());
//         }

//         // The contents don't fit in the write buffer, so we try flushing to the
//         // underlying stream. This helps prevent unnecessary growth of the write
//         // buffer at the expense of additional system calls.
//         match self.flush() {
//             Ok(()) => {
//                 // flush completed, we can now see if the contents are still
//                 // bigger than the write buffer. If they are, we attempt to
//                 // bypass the write buffer and write directly to the stream.
//                 if buf.len() >= self.write_buffer.remaining_mut() {
//                     // large write, attempt to bypass buffer
//                     match self.stream.write(buf) {
//                         Ok(n) => {
//                             if n == buf.len() {
//                                 // complete write
//                                 Ok(buf.len())
//                             } else {
//                                 // partial write, copy the rest to the buffer
//                                 self.write_buffer.put_slice(&buf[n..]);
//                                 Ok(buf.len())
//                             }
//                         }
//                         Err(e) => match e.kind() {
//                             ErrorKind::Interrupted | ErrorKind::WouldBlock => {
//                                 // no bytes were written, but could be written
//                                 // in the future, put them in the write buffer.
//                                 // NOTE: `Interrupted` is immediately retryable,
//                                 // but it's simpler to handle this way for now.
//                                 self.write_buffer.put_slice(buf);
//                                 Ok(buf.len())
//                             }
//                             _ => Err(e),
//                         },
//                     }
//                 } else {
//                     // small write, just write to buffer
//                     self.write_buffer.put_slice(buf);
//                     Ok(buf.len())
//                 }
//             }
//             Err(e) => match e.kind() {
//                 ErrorKind::Interrupted | ErrorKind::WouldBlock => {
//                     // flush shouldn't return interrupted, but both these errors
//                     // indicate that future writes may be successful. Write the
//                     // contents into the write buffer
//                     self.write_buffer.put_slice(buf);
//                     Ok(buf.len())
//                 }
//                 _ => {
//                     // flush failed in some way that is not retryable, bubble
//                     // the error up to the caller
//                     Err(e)
//                 }
//             },
//         }
//     }

//     // NOTE: this is implemented as a non-blocking operation that may make
//     // multiple syscalls to complete. An `Ok` result indicates that the entire
//     // write buffer has been flushed to the underlying stream. An `Err` result
//     // indicates that some or all of the write buffer was *not* flushed to the
//     // underlying stream and that flush should be called again in the future.
//     fn flush(&mut self) -> Result<()> {
//         while self.write_buffer.has_remaining() {
//             match self.stream.write(self.write_buffer.borrow()) {
//                 Ok(amt) => {
//                     self.write_buffer.advance(amt);
//                 }
//                 Err(e) => match e.kind() {
//                     ErrorKind::Interrupted => {
//                         // this should be retried immediately
//                     }
//                     _ => {
//                         // all other errors get bubbled up
//                         return Err(e);
//                     }
//                 },
//             }
//         }

//         Ok(())
//     }
// }

impl Borrow<[u8]> for Session {
    fn borrow(&self) -> &[u8] {
        self.read_buffer.borrow()
    }
}

impl Buf for Session {
    fn remaining(&self) -> usize {
        self.read_buffer.remaining()
    }

    fn chunk(&self) -> &[u8] {
        self.read_buffer.chunk()
    }

    fn advance(&mut self, amt: usize) {
        self.read_buffer.advance(amt)
    }
}

unsafe impl BufMut for Session {
    fn remaining_mut(&self) -> usize {
        self.write_buffer.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, amt: usize) {
        self.write_buffer.advance_mut(amt)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        self.write_buffer.chunk_mut()
    }

    #[allow(unused_mut)]
    fn put<T: Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        self.write_buffer.put(src)
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.write_buffer.put_slice(src)
    }
}

impl event::Source for Session {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> Result<()> {
        self.stream.register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> Result<()> {
        self.stream.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> Result<()> {
        self.stream.deregister(registry)
    }
}

/// A basic session to represent the client side of a framed session.
pub struct ClientSession<Parser, Rx, Tx> {
    session: Session,
    parser: Parser,
    pending: VecDeque<(Instant, Tx)>,
    _rx: PhantomData<Rx>,
}


impl<Parser, Rx, Tx> Debug for ClientSession<Parser, Rx, Tx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.session)
    }
}

impl<Parser, Rx, Tx> ClientSession<Parser, Rx, Tx>
where
    Tx: Compose,
    Parser: Parse<Rx>,
{
    pub fn new(session: Session, parser: Parser) -> Self {
        Self {
            session,
            parser,
            pending: VecDeque::with_capacity(256),
            _rx: PhantomData,
        }
    }

    /// Sends the frame to the underlying session and attempts to flush the
    /// session buffer. This function also adds a timestamp to a queue so that
    /// response latencies can be determined. The latency will include any time
    /// that it takes to compose the message onto the session buffer, time to
    /// flush the session buffer, and any additional calls to flush which may be
    /// required.
    pub fn send(&mut self, tx: Tx) -> Result<usize> {
        let now = Instant::now();
        let size = tx.compose(&mut self.session);
        self.pending.push_back((now, tx));
        self.session.flush()?;
        Ok(size)
    }

    pub fn receive(&mut self) -> std::result::Result<(Tx, Rx), ParseError> {
        let src: &[u8] = self.session.borrow();
        match self.parser.parse(src) {
            Ok(res) => {
                let now = Instant::now();
                let (timestamp, request) = self.pending.pop_front().ok_or(
                    ParseError::Unknown
                )?;
                let _elapsed = now - timestamp;
                let consumed = res.consumed();
                let msg = res.into_inner();
                self.session.consume(consumed);
                Ok((request, msg))
            }
            Err(e) => Err(e),
        }
    }

    pub fn flush(&mut self) {
        let _ = self.session.flush();
    }
}

pub struct ServerSession<Parser, Rx, Tx> {
    session: Session,
    parser: Parser,
    pending: VecDeque<Instant>,
    outstanding: VecDeque<(Option<Instant>, usize)>,
    _rx: PhantomData<Rx>,
    _tx: PhantomData<Tx>,
}

impl<Parser, Rx, Tx> Debug for ServerSession<Parser, Rx, Tx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.session)
    }
}

impl<Parser, Rx, Tx> ServerSession<Parser, Rx, Tx>
where
    Tx: Compose,
    Parser: Parse<Rx>,
{
    pub fn new(session: Session, parser: Parser) -> Self {
        Self {
            session,
            parser,
            pending: VecDeque::with_capacity(256),
            outstanding: VecDeque::with_capacity(256),
            _rx: PhantomData,
            _tx: PhantomData,
        }
    }

    pub fn receive(&mut self) -> std::result::Result<Rx, ParseError> {
        let src: &[u8] = self.session.borrow();
        match self.parser.parse(src) {
            Ok(res) => {
                let now = Instant::now();
                self.pending.push_back(now);
                let consumed = res.consumed();
                let msg = res.into_inner();
                self.session.consume(consumed);
                Ok(msg)
            }
            Err(e) => Err(e),
        }
    }

    pub fn send(&mut self, tx: Tx) -> Result<usize> {
        let timestamp = self.pending.pop_front();

        let size = tx.compose(&mut self.session);

        if size == 0 {
            // we have a zero sized response
            if let Some(timestamp) = timestamp {
                let now = Instant::now();
                let _latency = now - timestamp;
            }
        } else {
            self.outstanding.push_back((timestamp, size));
            let _ = self.flush()?;
        }

        Ok(size)
    }

    pub fn flush(&mut self) -> Result<()> {
        let current_pending = self.session.write_pending();
        self.session.flush()?;
        let final_pending = self.session.write_pending();

        let mut flushed = current_pending - final_pending;

        if flushed == 0 {
            return Ok(());
        }

        let now = Instant::now();

        while flushed > 0 {
            if let Some(mut front) = self.outstanding.pop_front() {
                if front.1 > flushed {
                    front.1 -= flushed;
                    self.outstanding.push_front(front);
                    break;
                } else {
                    flushed -= front.1;
                    if let Some(ts) = front.0 {
                        let _latency = now - ts;
                    }
                }
            } else {
                break;
            }
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

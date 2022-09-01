// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Abstractions for bi-directional buffered communications on top of streams.
//! This allows for efficient reading and writing for stream-oriented
//! communication and provides abstractions for request/response oriented
//! client/server communications.

// pub use buffer::*;

#[macro_use]
extern crate log;

mod buffer;
mod client;
mod server;

pub use buffer::*;
pub use client::ClientSession;
pub use server::ServerSession;

use std::os::unix::prelude::AsRawFd;

use ::net::*;
use core::borrow::{Borrow, BorrowMut};
use core::fmt::Debug;
use core::marker::PhantomData;
use protocol_common::Compose;
use protocol_common::Parse;
use rustcommon_metrics::*;
use rustcommon_time::Nanoseconds;
use std::collections::VecDeque;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Result;
use std::io::Write;

const ONE_SECOND: u64 = 1_000_000_000; // in nanoseconds

gauge!(
    SESSION_BUFFER_BYTE,
    "current size of the session buffers in bytes"
);

counter!(SESSION_RECV, "number of reads from sessions");
counter!(
    SESSION_RECV_EX,
    "number of exceptions while reading from sessions"
);
counter!(SESSION_RECV_BYTE, "number of bytes read from sessions");
counter!(SESSION_SEND, "number of writes to sessions");
counter!(
    SESSION_SEND_EX,
    "number of exceptions while writing to sessions"
);
counter!(SESSION_SEND_BYTE, "number of bytes written to sessions");

heatmap!(
    REQUEST_LATENCY,
    ONE_SECOND,
    "distribution of request latencies in nanoseconds"
);

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

impl AsRawFd for Session {
    fn as_raw_fd(&self) -> i32 {
        self.stream.as_raw_fd()
    }
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
    pub fn interest(&mut self) -> Interest {
        if self.write_buffer.has_remaining() {
            self.stream.interest().add(Interest::WRITABLE)
        } else {
            self.stream.interest()
        }
    }

    /// Indicates if the `Session` can be considered established, meaning that
    /// any underlying stream negotation and handshaking is completed.
    pub fn is_established(&mut self) -> bool {
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

    /// Mark `amt` bytes as consumed from the read buffer.
    pub fn consume(&mut self, amt: usize) {
        self.read_buffer.advance(amt)
    }

    /// Return the number of bytes currently in the write buffer.
    pub fn write_pending(&self) -> usize {
        self.write_buffer.remaining()
    }

    /// Attempts to flush the `Session` to the underlying `Stream`. This may
    /// result in multiple calls
    pub fn flush(&mut self) -> Result<usize> {
        let mut flushed = 0;
        while self.write_buffer.has_remaining() {
            match self.stream.write(self.write_buffer.borrow()) {
                Ok(amt) => {
                    // successfully wrote `amt` bytes to the stream, advance the
                    // write buffer and increment the flushed stat
                    self.write_buffer.advance(amt);
                    flushed += amt;
                }
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => {
                        // returns `WouldBlock` if this is the first time
                        if flushed == 0 {
                            return Err(e);
                        }
                        // otherwise, break from the loop and return the amount
                        // written until now
                        break;
                    }
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

        SESSION_SEND_BYTE.add(flushed as _);

        Ok(flushed)
    }

    pub fn do_handshake(&mut self) -> Result<()> {
        self.stream.do_handshake()
    }

    pub fn read_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.read_buffer
    }

    pub fn write_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.write_buffer
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

impl From<TcpStream> for Session {
    fn from(other: TcpStream) -> Self {
        Self::new(
            Stream::from(other),
            Buffer::new(TARGET_READ_SIZE),
            Buffer::new(TARGET_READ_SIZE),
        )
    }
}

impl Borrow<[u8]> for Session {
    fn borrow(&self) -> &[u8] {
        self.read_buffer.borrow()
    }
}

impl Borrow<[u8]> for &mut Session {
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
    fn register(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.stream.register(registry, token, interest)
    }

    fn reregister(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.stream.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> Result<()> {
        self.stream.deregister(registry)
    }
}

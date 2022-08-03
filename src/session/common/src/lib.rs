// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use buffer::*;

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

counter!(CLIENT_SESSION_RECV);
counter!(CLIENT_SESSION_RECV_EX);
counter!(CLIENT_SESSION_SEND);
counter!(CLIENT_SESSION_SEND_EX);
heatmap!(CLIENT_RESPONSE_LATENCY, ONE_SECOND);

counter!(SERVER_SESSION_READ);
counter!(SERVER_SESSION_READ_BYTES);
counter!(SERVER_SESSION_READ_EX);
counter!(SERVER_SESSION_RECV);
counter!(SERVER_SESSION_RECV_EX);
counter!(SERVER_SESSION_SEND);
counter!(SERVER_SESSION_SEND_EX);
heatmap!(SERVER_RESPONSE_LATENCY, ONE_SECOND);

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

    pub fn do_handshake(&mut self) -> Result<()> {
        self.stream.do_handshake()
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

/// A basic session to represent the client side of a framed session.
pub struct ClientSession<Parser, Tx, Rx> {
    session: Session,
    parser: Parser,
    pending: VecDeque<(Instant, Tx)>,
    _rx: PhantomData<Rx>,
}

impl<Parser, Tx, Rx> Debug for ClientSession<Parser, Tx, Rx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.session)
    }
}

impl<Parser, Tx, Rx> ClientSession<Parser, Tx, Rx>
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
        CLIENT_SESSION_SEND.increment();
        let now = Instant::now();
        let size = tx.compose(&mut self.session);
        self.pending.push_back((now, tx));
        self.session.flush()?;
        Ok(size)
    }

    pub fn receive(&mut self) -> Result<(Tx, Rx)> {
        let src: &[u8] = self.session.borrow();
        match self.parser.parse(src) {
            Ok(res) => {
                CLIENT_SESSION_RECV.increment();
                let now = Instant::now();
                let (timestamp, request) = self
                    .pending
                    .pop_front()
                    .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
                let latency = now - timestamp;
                CLIENT_RESPONSE_LATENCY.increment(now, latency.as_nanos(), 1);
                let consumed = res.consumed();
                let msg = res.into_inner();
                self.session.consume(consumed);
                Ok((request, msg))
            }
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                    CLIENT_SESSION_RECV_EX.increment();
                }
                Err(e)
            }
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        self.session.flush()?;
        Ok(())
    }

    pub fn write_pending(&self) -> usize {
        self.session.write_pending()
    }

    pub fn fill(&mut self) -> Result<usize> {
        self.session.fill()
    }

    pub fn interest(&self) -> Interest {
        self.session.interest()
    }

    pub fn do_handshake(&mut self) -> Result<()> {
        self.session.do_handshake()
    }
}

impl<Parser, Tx, Rx> Borrow<[u8]> for ClientSession<Parser, Tx, Rx> {
    fn borrow(&self) -> &[u8] {
        self.session.borrow()
    }
}

impl<Parser, Tx, Rx> Buf for ClientSession<Parser, Tx, Rx> {
    fn remaining(&self) -> usize {
        self.session.remaining()
    }

    fn chunk(&self) -> &[u8] {
        self.session.chunk()
    }

    fn advance(&mut self, amt: usize) {
        self.session.advance(amt)
    }
}

unsafe impl<Parser, Tx, Rx> BufMut for ClientSession<Parser, Tx, Rx> {
    fn remaining_mut(&self) -> usize {
        self.session.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, amt: usize) {
        self.session.advance_mut(amt)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        self.session.chunk_mut()
    }

    #[allow(unused_mut)]
    fn put<T: Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        self.session.put(src)
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.session.put_slice(src)
    }
}

impl<Parser, Tx, Rx> event::Source for ClientSession<Parser, Tx, Rx> {
    fn register(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.session.register(registry, token, interest)
    }

    fn reregister(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.session.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> Result<()> {
        self.session.deregister(registry)
    }
}

pub struct ServerSession<Parser, Tx, Rx> {
    session: Session,
    parser: Parser,
    pending: VecDeque<Instant>,
    outstanding: VecDeque<(Option<Instant>, usize)>,
    _rx: PhantomData<Rx>,
    _tx: PhantomData<Tx>,
}

impl<Parser, Tx, Rx> Debug for ServerSession<Parser, Tx, Rx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.session)
    }
}

impl<Parser, Tx, Rx> ServerSession<Parser, Tx, Rx>
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

    pub fn into_inner(self) -> Session {
        self.session
    }

    pub fn receive(&mut self) -> Result<Rx> {
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
        SERVER_SESSION_SEND.increment();

        let timestamp = self.pending.pop_front();

        let size = tx.compose(&mut self.session);

        if size == 0 {
            // we have a zero sized response, increment heatmap now
            if let Some(timestamp) = timestamp {
                let now = Instant::now();
                let latency = now - timestamp;
                SERVER_RESPONSE_LATENCY.increment(now, latency.as_nanos(), 1);
            }
        } else {
            // we have bytes in our response, we need to add it on the
            // outstanding response queue
            self.outstanding.push_back((timestamp, size));
            if let Err(e) = self.flush() {
                if e.kind() != ErrorKind::WouldBlock {
                    SERVER_SESSION_SEND_EX.increment();
                }
                return Err(e);
            }
        }

        Ok(size)
    }

    /// Attempts to flush all bytes currently in the write buffer to the
    /// underlying stream. Also handles bookeeping necessary to determine the
    /// server-side response latency.
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
                        let latency = now - ts;
                        SERVER_RESPONSE_LATENCY.increment(now, latency.as_nanos(), 1);
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Returns the number of bytes pending in the write buffer.
    pub fn write_pending(&self) -> usize {
        self.session.write_pending()
    }

    /// Reads from the underlying stream and returns the number of bytes read.
    pub fn fill(&mut self) -> Result<usize> {
        SERVER_SESSION_READ.increment();

        match self.session.fill() {
            Ok(amt) => {
                SERVER_SESSION_READ_BYTES.add(amt as _);
                Ok(amt)
            }
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                    SERVER_SESSION_READ_EX.increment();
                }
                Err(e)
            }
        }
    }

    pub fn interest(&self) -> Interest {
        self.session.interest()
    }

    pub fn do_handshake(&mut self) -> Result<()> {
        self.session.do_handshake()
    }
}

impl<Parser, Tx, Rx> Borrow<[u8]> for ServerSession<Parser, Tx, Rx> {
    fn borrow(&self) -> &[u8] {
        self.session.borrow()
    }
}

impl<Parser, Tx, Rx> Buf for ServerSession<Parser, Tx, Rx> {
    fn remaining(&self) -> usize {
        self.session.remaining()
    }

    fn chunk(&self) -> &[u8] {
        self.session.chunk()
    }

    fn advance(&mut self, amt: usize) {
        self.session.advance(amt)
    }
}

unsafe impl<Parser, Tx, Rx> BufMut for ServerSession<Parser, Tx, Rx> {
    fn remaining_mut(&self) -> usize {
        self.session.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, amt: usize) {
        self.session.advance_mut(amt)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        self.session.chunk_mut()
    }

    #[allow(unused_mut)]
    fn put<T: Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        self.session.put(src)
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.session.put_slice(src)
    }
}

impl<Parser, Tx, Rx> event::Source for ServerSession<Parser, Tx, Rx> {
    fn register(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.session.register(registry, token, interest)
    }

    fn reregister(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.session.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> Result<()> {
        self.session.deregister(registry)
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

// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate provides buffered TCP sessions with or without TLS which can be
//! used with [`::mio`]. TLS/SSL is provided by BoringSSL with the [`::boring`]
//! crate.

#[macro_use]
extern crate logger;

mod buffer;
mod stream;
// mod tcp_stream;
use common::ssl::{MidHandshakeSslStream, SslStream};
use net::event::Source;
use net::{Interest, Poll, Token};
use rustcommon_metrics::{counter, gauge, heatmap, metric, Counter, Gauge, Heatmap, Relaxed};
use std::borrow::{Borrow, BorrowMut};
use std::cmp::Ordering;
use std::io::{BufRead, ErrorKind, Read, Write};
use std::net::SocketAddr;

pub use buffer::Buffer;
use stream::Stream;

type Instant = common::time::Instant<common::time::Nanoseconds<u64>>;

// pub use tcp_stream::TcpStream;

pub use net::TcpStream;

gauge!(
    SESSION_BUFFER_BYTE,
    "current size of the session buffers in bytes"
);

counter!(
    TCP_ACCEPT,
    "number of times accept has been called on listening sockets"
);
counter!(TCP_CLOSE, "number of times TCP streams have been closed");
gauge!(TCP_CONN_CURR, "current number of open TCP streams");
counter!(TCP_RECV_BYTE, "number of bytes received on TCP streams");
counter!(TCP_SEND_BYTE, "number of bytes sent on TCP streams");
counter!(
    TCP_SEND_PARTIAL,
    "number of partial writes to the system socket buffer"
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
    1_000_000_000,
    "distribution of request latencies in nanoseconds"
);
heatmap!(
    PIPELINE_DEPTH,
    100_000,
    "distribution of request pipeline depth"
);

// TODO(bmartin): implement connect/reconnect so we can use this in clients too.
/// The core `Session` type which represents a TCP stream (with or without TLS),
/// the session buffer, the mio [`::mio::Token`],
pub struct Session {
    token: Token,
    stream: Stream,
    read_buffer: Buffer,
    write_buffer: Buffer,
    min_capacity: usize,
    max_capacity: usize,
    // hold current interest set
    interest: Interest,
    // TODO(bmartin): consider moving these fields and associated logic
    // out into a response tracking struct. It would make the session
    // type more applicable to clients if we move this out.
    //
    /// A timestamp which is used to calculate response latency
    timestamp: Instant,
    /// This is a queue of pending response sizes. When a response is finalized,
    /// the bytes in that response are pushed onto the back of the queue. As the
    /// session flushes out to the underlying socket, we can calculate when a
    /// response is completely flushed to the underlying socket and record a
    /// response latency.
    pending_responses: [usize; 256],
    /// This is the index of the first pending response.
    pending_head: usize,
    /// This is the count of pending responses.
    pending_count: usize,
    /// This holds the total number of bytes pending for finalized responses. By
    /// tracking this, we can determine the size of a response even if it is
    /// written into the session with multiple calls to write. It is essentially
    /// a cached value of `write_buffer.pending_bytes()` that does not reflect
    /// bytes from responses which are not yet finalized.
    pending_bytes: usize,
    /// This tracks the pipeline depth by tracking the number of responses
    /// between resets of the session timestamp.
    processed: usize,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        if let Ok(peer_addr) = self.peer_addr() {
            write!(f, "{}", peer_addr)
        } else {
            write!(f, "no peer address")
        }
    }
}

impl Session {
    /// Create a new `Session` with  representing a plain `TcpStream` with
    /// internal buffers which can hold up to capacity bytes without
    /// reallocating.
    pub fn plain_with_capacity(
        stream: TcpStream,
        min_capacity: usize,
        max_capacity: usize,
    ) -> Self {
        Self::new(Stream::plain(stream), min_capacity, max_capacity)
    }

    /// Create a new `Session` representing a negotiated `SslStream`
    pub fn tls_with_capacity(
        stream: SslStream<TcpStream>,
        min_capacity: usize,
        max_capacity: usize,
    ) -> Self {
        Self::new(Stream::tls(stream), min_capacity, max_capacity)
    }

    /// Create a new `Session` representing a `MidHandshakeSslStream`
    pub fn handshaking_with_capacity(
        stream: MidHandshakeSslStream<TcpStream>,
        min_capacity: usize,
        max_capacity: usize,
    ) -> Self {
        Self::new(Stream::handshaking(stream), min_capacity, max_capacity)
    }

    /// Create a new `Session`
    fn new(stream: Stream, min_capacity: usize, max_capacity: usize) -> Self {
        TCP_ACCEPT.increment();
        TCP_CONN_CURR.add(1);
        Self {
            token: Token(0),
            stream,
            read_buffer: Buffer::with_capacity(min_capacity),
            write_buffer: Buffer::with_capacity(min_capacity),
            min_capacity,
            max_capacity,
            interest: Interest::READABLE,
            timestamp: Instant::now(),
            pending_responses: [0; 256],
            pending_head: 0,
            pending_count: 0,
            pending_bytes: 0,
            processed: 0,
        }
    }

    /// Register the `Session` with the event loop
    pub fn register(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        self.stream.register(poll.registry(), self.token, interest)
    }

    /// Deregister the `Session` from the event loop
    pub fn deregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        self.stream.deregister(poll.registry())
    }

    /// Reregister the `Session` with the event loop
    pub fn reregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        if interest == self.interest {
            return Ok(());
        }
        debug!("reregister: {:?}", interest);
        self.interest = interest;
        self.stream
            .reregister(poll.registry(), self.token, interest)
    }

    /// Get the token which is used with the event loop
    pub fn token(&self) -> Token {
        self.token
    }

    /// Set the token which is used with the event loop
    pub fn set_token(&mut self, token: Token) {
        self.token = token;
    }

    /// Get the set of readiness events the session is waiting for
    ///
    /// NOTE: we effectively block additional reads when there are writes
    /// pending. This may not be an appropriate choice for all use-cases, but
    /// for a server, it can be used to apply back-pressure.
    //
    // TODO(bmartin): we could make this behavior conditional if we have a
    // use-case that requires different handling, but it comes with complexity
    // of having to set the behavior for each session.
    fn readiness(&self) -> Interest {
        if self.write_buffer.is_empty() {
            Interest::READABLE
        } else {
            Interest::WRITABLE
        }
    }

    /// Returns a boolean which indicates if the session is handshaking
    pub fn is_handshaking(&self) -> bool {
        self.stream.is_handshaking()
    }

    /// Drives the handshake for the session. A successful result indicates that
    /// the session hadshake is completed successfully. The error result should
    /// be checked to determine if the operation would block, resulted in some
    /// unrecoverable error, or if the session was not in a handshaking state
    /// when this was called.
    pub fn do_handshake(&mut self) -> Result<(), std::io::Error> {
        self.stream.do_handshake()
    }

    /// Closes the session and the underlying stream.
    pub fn close(&mut self) {
        self.stream.close();
    }

    /// Returns the number of bytes in the read buffer
    pub fn read_pending(&self) -> usize {
        self.read_buffer.len()
    }

    /// Returns the number of bytes in the write buffer
    pub fn write_pending(&self) -> usize {
        self.write_buffer.len()
    }

    /// Returns the number of bytes free in the write buffer relative to the
    /// minimum buffer size. This allows us to use it as a signal that we should
    /// apply some backpressure on handling requests for the session.
    pub fn write_capacity(&self) -> usize {
        self.min_capacity.saturating_sub(self.write_pending())
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike [`fill_buf`], this will not attempt to fill the buffer if it is
    /// empty.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    pub fn buffer(&self) -> &[u8] {
        self.read_buffer.borrow()
    }

    pub fn peer_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.stream.peer_addr()
    }

    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }

    pub fn set_timestamp(&mut self, timestamp: Instant) {
        if self.processed > 0 {
            PIPELINE_DEPTH.increment(self.timestamp, self.processed as _, 1);
            self.processed = 0;
        }
        self.timestamp = timestamp;
    }

    pub fn finalize_response(&mut self) {
        self.processed += 1;
        let previous = self.pending_bytes;
        let current = self.write_pending();

        match current.cmp(&previous) {
            Ordering::Greater => {
                // We've finalized a response that has some pending bytes to
                // track. If there's room in the tracking struct, we add it so
                // we can determine latency later.
                if self.pending_count < self.pending_responses.len() {
                    let mut idx = self.pending_head + self.pending_count;
                    if idx >= self.pending_responses.len() {
                        idx %= self.pending_responses.len();
                    }
                    self.pending_responses[idx] = current - previous;
                    self.pending_count += 1;
                }
            }
            Ordering::Equal => {
                // We've finalized a response that is zero-length. This is
                // expected for empty responses such as when handling memcache
                // requests which specify `NOREPLY`. Since there are no pending
                // bytes for a zero-length response, we can determine the
                // latency now.
                let now = Instant::now();
                let latency = (now - self.timestamp()).as_nanos() as u64;
                REQUEST_LATENCY.increment(now, latency, 1);
            }
            Ordering::Less => {
                // This indicates that our tracking is off. This could be due to
                // a protocol failing to finalize some type of response.
                //
                // NOTE: this does not indicate corruption of the buffer and
                // only indicates some issue with the pending response tracking
                // used to calculate latencies. This path is an attempt to
                // recover by skipping the tracking for this request.
                error!(
                    "Failed to calculate length of finalized response. \
                    Previous pending bytes: {} Current write buffer length: {}",
                    previous, current
                );

                // If it's a debug build, we will also assert that this is
                // unexpected.
                debug_assert!(false);
            }
        }

        self.pending_bytes = current;
    }
}

impl Read for Session {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.read_buffer.is_empty() {
            self.fill_buf()?;
        }
        let bytes = std::cmp::min(buf.len(), self.read_buffer.len());
        let buffer: &[u8] = self.read_buffer.borrow();
        buf[0..bytes].copy_from_slice(&buffer[0..bytes]);
        self.consume(bytes);
        Ok(bytes)
    }
}

impl BufRead for Session {
    fn fill_buf(&mut self) -> Result<&[u8], std::io::Error> {
        SESSION_RECV.increment();
        let mut total_bytes = 0;
        loop {
            if self.read_buffer.len() == self.max_capacity {
                return Err(std::io::Error::new(ErrorKind::Other, "buffer full"));
            }

            // reserve additional space in the buffer if needed
            if self.read_buffer.available_capacity() == 0 {
                self.read_buffer.reserve(self.min_capacity);
            }

            match self.stream.read(self.read_buffer.borrow_mut()) {
                Ok(0) => {
                    // Stream is disconnected, stop reading
                    break;
                }
                Ok(bytes) => {
                    self.read_buffer.increase_len(bytes);
                    total_bytes += bytes;
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        // check if we blocked on first read or subsequent read.
                        // if blocked on a subsequent read, we stop reading and
                        // allow the function to return the number of bytes read
                        // until now.
                        if total_bytes == 0 {
                            return Err(e);
                        } else {
                            break;
                        }
                    } else {
                        SESSION_RECV_EX.increment();
                        return Err(e);
                    }
                }
            }
        }
        SESSION_RECV_BYTE.add(total_bytes as _);
        Ok(self.read_buffer.borrow())
    }

    fn consume(&mut self, amt: usize) {
        self.read_buffer.consume(amt);
    }
}

impl Write for Session {
    fn write(&mut self, src: &[u8]) -> Result<usize, std::io::Error> {
        self.write_buffer.reserve(src.len());
        self.write_buffer.extend_from_slice(src);
        Ok(src.len())
    }

    // need a different flush
    fn flush(&mut self) -> Result<(), std::io::Error> {
        SESSION_SEND.increment();
        match self.stream.write((self.write_buffer).borrow()) {
            Ok(0) => Ok(()),
            Ok(mut bytes) => {
                let flushed_bytes = bytes;
                SESSION_SEND_BYTE.add(bytes as _);
                self.write_buffer.consume(bytes);

                // NOTE: we expect that the stream flush is essentially a no-op
                // based on the implementation for `TcpStream`

                let now = Instant::now();
                let latency = (now - self.timestamp()).as_nanos() as u64;
                let mut completed = 0;

                // iterate through the pending response lengths and perform the
                // bookkeeping to calculate how many have been flushed to the
                // `TcpStream` in this call of `flush()`
                while bytes > 0 && self.pending_count > 0 {
                    // first response out of the buffer
                    let head = &mut self.pending_responses[self.pending_head];

                    if bytes >= *head {
                        // we flushed all (or more) than the first response
                        bytes -= *head;
                        *head = 0;
                        completed += 1;
                        self.pending_count -= 1;

                        // move the head pointer forward
                        if self.pending_head + 1 < self.pending_responses.len() {
                            self.pending_head += 1;
                        } else {
                            self.pending_head = 0;
                        }
                    } else {
                        // we only flushed part of the first response
                        *head -= bytes;
                        bytes = 0;
                    }
                }

                match flushed_bytes.cmp(&self.pending_bytes) {
                    Ordering::Less => {
                        // The buffer is not completely flushed to the
                        // underlying stream, we will still have more pending
                        // bytes.
                        self.pending_bytes -= flushed_bytes;
                    }
                    Ordering::Equal => {
                        // The buffer is completely flushed. We have no more
                        // pending bytes.
                        self.pending_bytes = 0;
                    }
                    Ordering::Greater => {
                        // This indicates that the tracking is off. Potentially
                        // due to a protocol implementation that failed to
                        // finalize some response.
                        //
                        // NOTE: this does not indicate corruption of the buffer
                        // and only indicates some issue with the pending
                        // response tracking used to calculate latencies. This
                        // path is an attempt to recover and resume tracking by
                        // setting the pending bytes to the current write buffer
                        // length.
                        error!(
                            "Session flushed {} bytes, but only had {} pending bytes to track",
                            flushed_bytes, self.pending_bytes
                        );
                        self.pending_bytes = self.write_pending();

                        // If it's a debug build, we will also assert that this
                        // is unexpected.
                        debug_assert!(false);
                    }
                }

                // Increment the histogram with the calculated latency.
                REQUEST_LATENCY.increment(now, latency, completed);

                Ok(())
            }
            Err(e) => {
                SESSION_SEND_EX.increment();
                Err(e)
            }
        }
    }
}

common::metrics::test_no_duplicates!();

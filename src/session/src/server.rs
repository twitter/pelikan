// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

/// A basic session to represent the server side of a framed session, meaning
/// that is is used by a server to talk to a client.
///
/// `ServerSession` latency tracking counts the time from data being read into
/// the session buffer until a corresponding message is flushed entirely to the
/// socket buffer. This means that if it takes multiple reads of the socket
/// buffer until a message from the client is parsable, the time of the last
/// read is the start of our latency tracking. Similarly, if it takes multiple
/// writes to flush a message back to the client, the time of the last write to
/// the socket buffer is the stop point for latency tracking. This means that
/// the server latency will be lower than that seen by the client, as delays in
/// the kernel, network, and client itself do not count towards server latency.
/// Instead, the latency represents the time it takes to parse the message, take
/// some possible action, and write a response back out to the socket buffer.
pub struct ServerSession<Parser, Tx, Rx> {
    // the actual session
    session: Session,
    // a parser which produces requests from the session buffer
    parser: Parser,
    // tracks the timestamps of any pending requests
    pending: VecDeque<Instant>,
    // tracks outstanding responses and the number of bytes remaining for each
    outstanding: VecDeque<(Option<Instant>, usize)>,
    // tracks the time the session buffer was last filled
    timestamp: Instant,
    // markers for the receive and transmit types
    _rx: PhantomData<Rx>,
    _tx: PhantomData<Tx>,
}

impl<Parser, Tx, Rx> AsRawFd for ServerSession<Parser, Tx, Rx> {
    fn as_raw_fd(&self) -> i32 {
        self.session.as_raw_fd()
    }
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
    // Create a new `ServerSession` from a `Session` and a `Parser`
    pub fn new(session: Session, parser: Parser) -> Self {
        Self {
            session,
            parser,
            pending: VecDeque::with_capacity(256),
            outstanding: VecDeque::with_capacity(256),
            timestamp: Instant::now(),
            _rx: PhantomData,
            _tx: PhantomData,
        }
    }

    /// Consume the `ServerSession` and return the inner `Session`
    pub fn into_inner(self) -> Session {
        self.session
    }

    /// Attempt to receive a single message from the current session buffer.
    pub fn receive(&mut self) -> Result<Rx> {
        let src: &[u8] = self.session.borrow();
        match self.parser.parse(src) {
            Ok(res) => {
                self.pending.push_back(self.timestamp);
                let consumed = res.consumed();
                let msg = res.into_inner();
                self.session.consume(consumed);
                Ok(msg)
            }
            Err(e) => Err(e),
        }
    }

    /// Send a message to the session buffer.
    pub fn send(&mut self, tx: Tx) -> Result<usize> {
        SESSION_SEND.increment();

        let timestamp = self.pending.pop_front();

        let size = tx.compose(&mut self.session);

        if size == 0 {
            // we have a zero sized response, increment heatmap now
            if let Some(timestamp) = timestamp {
                let now = Instant::now();
                let latency = now - timestamp;
                REQUEST_LATENCY.increment(now, latency.as_nanos(), 1);
            }
        } else {
            // we have bytes in our response, we need to add it on the
            // outstanding response queue
            self.outstanding.push_back((timestamp, size));
        }

        Ok(size)
    }

    /// Advances the read pointer for the session write buffer by `amt` bytes.
    /// This is used to mark the data as sent to the underlying session.
    pub fn advance_write(&mut self, amt: usize) {
        if amt == 0 {
            return;
        }

        let now = Instant::now();

        let mut amt = amt;

        while amt > 0 {
            if let Some(mut front) = self.outstanding.pop_front() {
                if front.1 > amt {
                    front.1 -= amt;
                    self.outstanding.push_front(front);
                    break;
                } else {
                    amt -= front.1;
                    if let Some(ts) = front.0 {
                        let latency = now - ts;
                        REQUEST_LATENCY.increment(now, latency.as_nanos(), 1);
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Attempts to flush all bytes currently in the write buffer to the
    /// underlying stream. Also handles bookeeping necessary to determine the
    /// server-side response latency.
    pub fn flush(&mut self) -> Result<()> {
        let current_pending = self.session.write_pending();
        self.session.flush()?;
        let final_pending = self.session.write_pending();

        let flushed = current_pending - final_pending;

        self.advance_write(flushed);

        Ok(())
    }

    /// Returns the number of bytes pending in the write buffer.
    pub fn write_pending(&self) -> usize {
        self.session.write_pending()
    }

    /// Reads from the underlying stream into the read buffer and returns the
    /// number of bytes read.
    pub fn fill(&mut self) -> Result<usize> {
        SESSION_RECV.increment();
        self.timestamp = Instant::now();

        match self.session.fill() {
            Ok(amt) => {
                SESSION_RECV_BYTE.add(amt as _);
                Ok(amt)
            }
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                    SESSION_RECV_EX.increment();
                }
                Err(e)
            }
        }
    }

    /// Returns the current event interest for this session.
    pub fn interest(&mut self) -> Interest {
        self.session.interest()
    }

    /// Attempt to handshake the underlying session.
    pub fn do_handshake(&mut self) -> Result<()> {
        self.session.do_handshake()
    }

    /// Get direct access to the read buffer.
    pub fn read_buffer_mut(&mut self) -> &mut Buffer {
        self.session.read_buffer_mut()
    }

    /// Get direct access to the write buffer.
    pub fn write_buffer_mut(&mut self) -> &mut Buffer {
        self.session.write_buffer_mut()
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

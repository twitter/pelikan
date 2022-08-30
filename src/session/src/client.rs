// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

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

impl<Parser, Tx, Rx> AsRawFd for ClientSession<Parser, Tx, Rx> {
    fn as_raw_fd(&self) -> i32 {
        self.session.as_raw_fd()
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

    /// Sends the frame to the underlying session but does *not* flush the
    /// session buffer. This function also adds a timestamp to a queue so that
    /// response latencies can be determined. The latency will include any time
    /// that it takes to compose the message onto the session buffer, time to
    /// flush the session buffer, and any additional calls to flush which may be
    /// required.
    pub fn send(&mut self, tx: Tx) -> Result<usize> {
        SESSION_SEND.increment();
        let now = Instant::now();
        let size = tx.compose(&mut self.session);
        self.pending.push_back((now, tx));
        Ok(size)
    }

    pub fn receive(&mut self) -> Result<(Tx, Rx)> {
        let src: &[u8] = self.session.borrow();
        match self.parser.parse(src) {
            Ok(res) => {
                SESSION_RECV.increment();
                let now = Instant::now();
                let (timestamp, request) = self
                    .pending
                    .pop_front()
                    .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
                let latency = now - timestamp;
                REQUEST_LATENCY.increment(now, latency.as_nanos(), 1);
                let consumed = res.consumed();
                let msg = res.into_inner();
                self.session.consume(consumed);
                Ok((request, msg))
            }
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                    SESSION_RECV_EX.increment();
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

    pub fn read_buffer_mut(&mut self) -> &mut Buffer {
        self.session.read_buffer_mut()
    }

    pub fn write_buffer_mut(&mut self) -> &mut Buffer {
        self.session.write_buffer_mut()
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

use buffer::*;
use protocol_common::*;
use session_common::ServerSession;

use std::io::Result;
use std::os::unix::io::AsRawFd;

#[derive(Clone, Copy, Debug)]
pub enum State {
    Poll,
    Read,
    Write,
    Shutdown,
}

pub struct Session<Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
{
    inner: ServerSession<Parser, Response, Request>,
    state: State,
}

impl<Parser, Request, Response> Session<Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
{
    pub fn state(&self) -> State {
        self.state
    }

    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    pub fn read_buffer_mut(&mut self) -> &mut Buffer {
        self.inner.read_buffer_mut()
    }

    pub fn write_buffer_mut(&mut self) -> &mut Buffer {
        self.inner.write_buffer_mut()
    }

    pub fn receive(&mut self) -> Result<Request> {
        self.inner.receive()
    }

    pub fn send(&mut self, response: Response) -> Result<usize> {
        self.inner.send(response)
    }
}

impl<Parser, Request, Response> AsRawFd for Session<Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
{
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}

impl<Parser, Request, Response> From<ServerSession<Parser, Response, Request>> for Session<Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
{
    fn from(other: ServerSession<Parser, Response, Request>) -> Self {
        Self {
            inner: other,
            state: State::Poll,
        }
    }
}


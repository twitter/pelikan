use pelikan::protocol::Protocol;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Empty {}

pub trait ClosableStream {
    fn close(&self) -> std::io::Result<()>;
}

/// A type that accepts a certain type of request and
/// initializes a response.
pub trait Worker {
    /// The protocol for the wire format.
    type Protocol: Protocol;

    /// Per-connection state. This is not reinitialized
    /// for each request.
    type State: Default;

    /// Handle a single request and initialize a response.
    fn process_request(
        &self,
        req: &mut <Self::Protocol as Protocol>::Request,
        rsp: &mut <Self::Protocol as Protocol>::Response,
        state: &mut Self::State,
    ) -> WorkerAction;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum WorkerAction {
    // Nothing special - sends the response as normal
    None,
    // Close the connection
    Close,
    // Don't send a response
    NoResponse,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

impl Default for WorkerAction {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(unix)]
impl ClosableStream for tokio::net::UnixStream {
    fn close(&self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Both)
    }
}

impl ClosableStream for tokio::net::TcpStream {
    fn close(&self) -> std::io::Result<()> {
        use std::net::Shutdown;

        self.shutdown(Shutdown::Both)
    }
}

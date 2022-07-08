use boring::ssl::MidHandshakeSslStream;
use boring::ssl::SslStream;
use std::io::Read;
use std::io::Write;

struct Session<T, U> {
    stream: Stream<T>,
    read_buffer: U,
    write_buffer: U,
}

/// This provides a stream abstraction that may have TLS (SSL) encryption.
enum StreamType<T> {
    Plain(T),
    Ssl(SslStream<T>),
    MidHandshakeSsl(MidHandshakeSslStream<T>),
}

pub struct Stream<T> {
    inner: StreamType<T>,
    state: State,
}

pub enum State {
    Connecting,
    Handshaking,
    Established,
}

impl<T> Stream<T> where T: Read + Write {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

pub mod admin;
pub mod memcache;

use crate::buffer::Buffer;

pub const CRLF: &str = "\r\n";

pub trait Compose {
    fn compose(self, buffer: &mut dyn Buffer);
}

pub trait Execute<Request, Response> {
    fn execute(&mut self, request: Request) -> Response;
}

pub enum ParseError {
    Invalid,
    Incomplete,
    UnknownCommand,
}

pub trait Parse<Message> {
    fn parse(&mut self) -> Result<Message, ParseError>;
}

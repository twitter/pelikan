pub enum ParseError {
	Invalid,
	Incomplete,
	UnknownCommand,
}

pub trait Parse<Message> {
	fn parse(&mut self) -> Result<Message, ParseError>;
}

pub trait Execute<Request, Response> {
	fn execute(&mut self, request: Request) -> Response;
}

pub trait GetTtl {
	fn get_ttl(&self, expiry: u32) -> u32;
}
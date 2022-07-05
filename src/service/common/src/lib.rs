use protocol_common::*;
use session::Session;

pub trait Client<Request, Response> {
    fn recv(&self, src: &[u8], req: &Request) -> Result<ParseOk<Response>, ParseError>;
    fn send(&self, dst: &mut Session, req: &Request);
}

pub trait Server<Request, Response> {
    fn recv(&self, src: &[u8]) -> Result<ParseOk<Request>, ParseError>;
    fn send(&self, dst: &mut Session, req: Request, res: Response);
}

#[macro_export]
#[rustfmt::skip]
macro_rules! counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

#[cfg(not(any(feature = "namespace", all(feature = "client", feature = "server"))))]
pub use counter as client_counter;

#[cfg(any(feature = "namespace", all(feature = "client", feature = "server")))]
#[macro_export]
#[rustfmt::skip]
macro_rules! client_counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name,
            namespace = "client"
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            namespace = "client",
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

#[cfg(not(any(feature = "namespace", all(feature = "client", feature = "server"))))]
pub use counter as server_counter;

#[cfg(any(feature = "namespace", all(feature = "client", feature = "server")))]
#[macro_export]
#[rustfmt::skip]
macro_rules! server_counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name,
            namespace = "server"
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            namespace = "server",
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

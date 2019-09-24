use super::*;
use pelikan_sys::protocol::admin::{request, response};

pub enum AdminProtocol {}

impl Protocol for AdminProtocol {
    type Request = request;
    type Response = response;
}

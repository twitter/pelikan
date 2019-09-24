#[cfg(feature = "protocol_admin")]
pub mod admin;

/// Trait defining the request and response types for a protocol
pub trait Protocol {
    type Request;
    type Response;
}

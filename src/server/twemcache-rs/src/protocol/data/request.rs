use super::*;

// TODO(bmartin): this should be lifted out into a common crate and shared
// between different protocols
pub trait Request {
    type Command;

    fn command(&self) -> Self::Command;
    fn keys(&self) -> Vec<&[u8]>;
}

#[derive(Debug, PartialEq)]
/// The `MemcacheRequest` contains all the specific fields which represent a
/// memcache request.
pub struct MemcacheRequest {
    pub(super) buffer: BytesMut,
    pub(super) command: MemcacheCommand,
    pub(super) consumed: usize,
    pub(super) keys: Vec<(usize, usize)>,
    pub(super) noreply: bool,
    pub(super) expiry: u32,
    pub(super) flags: u32,
    pub(super) value: (usize, usize),
    pub(super) cas: u64,
}

impl MemcacheRequest {
    /// Should a reply be sent to the client for this request?
    pub fn noreply(&self) -> bool {
        self.noreply
    }

    /// Return the number of bytes consumed from the read buffer by the parsed
    /// request
    pub fn consumed(&self) -> usize {
        self.consumed
    }

    /// Return the expiry for the value
    pub fn expiry(&self) -> u32 {
        self.expiry
    }

    /// Optional flag which is stored as an opaque 32bit value
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// The associated value for the request
    pub fn value(&self) -> Option<&[u8]> {
        let start = self.value.0;
        let end = self.value.1;
        if start == end {
            None
        } else {
            Some(&self.buffer[start..end])
        }
    }

    /// The cas value for the request
    pub fn cas(&self) -> u64 {
        self.cas
    }
}

impl Request for MemcacheRequest {
    type Command = MemcacheCommand;

    fn command(&self) -> Self::Command {
        self.command
    }

    fn keys(&self) -> Vec<&[u8]> {
        let buffer: &[u8] = self.buffer.borrow();
        let mut keys = Vec::new();
        for key_index in &self.keys {
            keys.push(&buffer[key_index.0..key_index.1])
        }
        keys
    }
}
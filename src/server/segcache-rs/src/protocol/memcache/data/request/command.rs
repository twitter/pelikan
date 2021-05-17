use crate::protocol::ParseError;
use core::convert::TryFrom;

/// Memcache protocol commands
pub enum MemcacheCommand {
    Get,
    Gets,
    Set,
    Add,
    Replace,
    Delete,
    Cas,
}

impl TryFrom<&[u8]> for MemcacheCommand {
    type Error = ParseError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let cmd = match value {
            b"get" => MemcacheCommand::Get,
            b"gets" => MemcacheCommand::Gets,
            b"set" => MemcacheCommand::Set,
            b"add" => MemcacheCommand::Add,
            b"replace" => MemcacheCommand::Replace,
            b"cas" => MemcacheCommand::Cas,
            b"delete" => MemcacheCommand::Delete,
            _ => {
                return Err(ParseError::UnknownCommand);
            }
        };
        Ok(cmd)
    }
}

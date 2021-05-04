use segcache::Item;

use super::*;

pub enum MemcacheResponse {
    Deleted,
    End,
    Exists,
    Item { item: Item, cas: bool },
    NotFound,
    Stored,
    NotStored,
}

impl MemcacheResponse {
    pub fn serialize(self, buffer: &mut BytesMut) {
        match self {
            Self::Deleted => buffer.extend_from_slice(b"DELETED\r\n"),
            Self::End => buffer.extend_from_slice(b"END\r\n"),
            Self::Exists => buffer.extend_from_slice(b"EXISTS\r\n"),
            Self::Item { item, cas } => {
                buffer.extend_from_slice(b"VALUE ");
                buffer.extend_from_slice(item.key());
                let f = item.optional().unwrap();
                let flags: u32 = u32::from_be_bytes([f[0], f[1], f[2], f[3]]);
                if cas {
                    buffer.extend_from_slice(
                        format!(" {} {} {}", flags, item.value().len(), item.cas()).as_bytes(),
                    );
                } else {
                    buffer
                        .extend_from_slice(format!(" {} {}", flags, item.value().len()).as_bytes());
                }
                buffer.extend_from_slice(CRLF);
                buffer.extend_from_slice(item.value());
                buffer.extend_from_slice(CRLF);
            }
            Self::NotFound => buffer.extend_from_slice(b"NOT_FOUND\r\n"),
            Self::NotStored => buffer.extend_from_slice(b"NOT_STORED\r\n"),
            Self::Stored => buffer.extend_from_slice(b"STORED\r\n"),
        }
    }
}
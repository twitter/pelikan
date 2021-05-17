//! A very simple buffer type that can be replaced in the future.

pub struct Buffer {
    pub inner: BytesMut,
}

use bytes::BytesMut;
use std::borrow::Borrow;

impl Buffer {
    pub fn extend(&mut self, data: &[u8]) {
        self.inner.extend_from_slice(data)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: BytesMut::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn split_to(&mut self, index: usize) -> Self {
        Self {
            inner: self.inner.split_to(index),
        }
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        self.inner.borrow()
    }
}

// impl Parse<MemcacheRequest> for Buffer {
//     fn parse(&mut self) -> Result<MemcacheRequest, ParseError> {
//         match parse_command(self)? {
//             MemcacheCommand::Get => parse_get(self.inner),
//             MemcacheCommand::Gets => parse_gets(self.inner),
//             MemcacheCommand::Set => parse_set(self.inner),
//             MemcacheCommand::Add => parse_add(self.inner),
//             MemcacheCommand::Replace => parse_replace(self.inner),
//             MemcacheCommand::Cas => parse_cas(self.inner),
//             MemcacheCommand::Delete => parse_delete(self.inner),
//         }
//     }
// }

// fn parse_command(buffer: &mut BytesMut) -> Result<MemcacheCommand, ParseError> {
//     let command;
//     {
//         let buf: &[u8] = (*buffer).borrow();
//         // check if we got a CRLF
//         let mut double_byte = buf.windows(CRLF.len());
//         if let Some(_line_end) = double_byte.position(|w| w == CRLF.as_bytes()) {
//             // single-byte windowing to find spaces
//             let mut single_byte = buf.windows(1);
//             if let Some(cmd_end) = single_byte.position(|w| w == b" ") {
//                 command = MemcacheCommand::try_from(&buf[0..cmd_end])?;
//             } else {
//                 return Err(ParseError::Incomplete);
//             }
//         } else {
//             return Err(ParseError::Incomplete);
//         }
//     }
//     Ok(command)
// }

// #[allow(clippy::unnecessary_wraps)]
// fn parse_get(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let buf: &[u8] = (*buffer).borrow();

//     let mut double_byte = buf.windows(CRLF.len());
//     let line_end = double_byte.position(|w| w == CRLF.as_bytes()).unwrap();

//     let mut single_byte = buf.windows(1);
//     // we already checked for this in the MemcacheParser::parse()
//     let cmd_end = single_byte.position(|w| w == b" ").unwrap();
//     let mut previous = cmd_end + 1;
//     let mut keys = Vec::new();

//     // command may have multiple keys, we need to loop until we hit
//     // a CRLF
//     loop {
//         if let Some(key_end) = single_byte.position(|w| w == b" ") {
//             if key_end < line_end {
//                 keys.push(buffer[previous..key_end].to_vec().into_boxed_slice());
//                 previous = key_end + 1;
//             } else {
//                 keys.push(buffer[previous..line_end].to_vec().into_boxed_slice());
//                 break;
//             }
//         } else {
//             keys.push(buffer[previous..line_end].to_vec().into_boxed_slice());
//             break;
//         }
//     }

//     let consumed = line_end + CRLF.len();

//     let request = MemcacheRequest {
//         command: MemcacheCommand::Get,
//         keys: keys.into_boxed_slice(),
//         noreply: false,
//         expiry: 0,
//         flags: 0,
//         value: None,
//         cas: 0,
//     };
//     buffer.advance(consumed);
//     Ok(request)
// }

// fn parse_gets(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let mut request = parse_get(buffer)?;
//     request.command = MemcacheCommand::Gets;
//     Ok(request)
// }

// fn parse_set(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let buf: &[u8] = (*buffer).borrow();

//     let mut single_byte = buf.windows(1);
//     if let Some(cmd_end) = single_byte.position(|w| w == b" ") {
//         // key
//         let key_end = single_byte
//             .position(|w| w == b" ")
//             .ok_or(ParseError::Incomplete)?
//             + cmd_end
//             + 1;

//         // flags
//         let flags_end = single_byte
//             .position(|w| w == b" ")
//             .ok_or(ParseError::Incomplete)?
//             + key_end
//             + 1;
//         let flags_str =
//             std::str::from_utf8(&buf[(key_end + 1)..flags_end]).map_err(|_| ParseError::Invalid)?;
//         let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

//         // expiry
//         let expiry_end = single_byte
//             .position(|w| w == b" ")
//             .ok_or(ParseError::Incomplete)?
//             + flags_end
//             + 1;
//         let expiry_str = std::str::from_utf8(&buf[(flags_end + 1)..expiry_end])
//             .map_err(|_| ParseError::Invalid)?;
//         let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

//         // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
//         let mut double_byte = buf.windows(CRLF.len());
//         let mut noreply = false;

//         // get the position of the next space and first CRLF
//         let next_space = single_byte
//             .position(|w| w == b" ")
//             .map(|v| v + expiry_end + 1);
//         let first_crlf = double_byte
//             .position(|w| w == CRLF.as_bytes())
//             .ok_or(ParseError::Incomplete)?;

//         let bytes_end = if let Some(next_space) = next_space {
//             // if we have both, bytes_end is before the earlier of the two
//             if next_space < first_crlf {
//                 // validate that noreply isn't malformed
//                 if &buf[(next_space + 1)..(first_crlf)] == NOREPLY.as_bytes() {
//                     noreply = true;
//                     next_space
//                 } else {
//                     return Err(ParseError::Invalid);
//                 }
//             } else {
//                 first_crlf
//             }
//         } else {
//             first_crlf
//         };

//         // this checks for malformed requests where a CRLF is at an
//         // unexpected part of the request
//         if (expiry_end + 1) >= bytes_end {
//             return Err(ParseError::Invalid);
//         }

//         if let Ok(Ok(bytes)) =
//             std::str::from_utf8(&buf[(expiry_end + 1)..bytes_end]).map(|v| v.parse::<usize>())
//         {
//             let consumed = first_crlf + CRLF.len() + bytes + CRLF.len();
//             if buf.len() >= consumed {
//                 let request = MemcacheRequest {
//                     command: MemcacheCommand::Set,
//                     keys: vec![buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice()]
//                         .into_boxed_slice(),
//                     noreply,
//                     expiry,
//                     flags,
//                     value: Some(
//                         buffer[(first_crlf + CRLF.len())..(first_crlf + CRLF.len() + bytes)]
//                             .to_vec()
//                             .into_boxed_slice(),
//                     ),
//                     cas: 0,
//                 };
//                 buffer.advance(consumed);
//                 Ok(request)
//             } else {
//                 // the buffer doesn't yet have all the bytes for the value
//                 Err(ParseError::Incomplete)
//             }
//         } else {
//             // expiry couldn't be parsed
//             Err(ParseError::Invalid)
//         }
//     } else {
//         // no space (' ') in the buffer
//         Err(ParseError::Incomplete)
//     }
// }

// fn parse_add(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let mut request = parse_set(buffer)?;
//     request.command = MemcacheCommand::Add;
//     Ok(request)
// }

// fn parse_replace(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let mut request = parse_set(buffer)?;
//     request.command = MemcacheCommand::Replace;
//     Ok(request)
// }

// fn parse_cas(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let buf: &[u8] = (*buffer).borrow();

//     let mut single_byte = buf.windows(1);
//     // we already checked for this in the MemcacheParser::parse()
//     let cmd_end = single_byte.position(|w| w == b" ").unwrap();
//     let key_end = single_byte
//         .position(|w| w == b" ")
//         .ok_or(ParseError::Incomplete)?
//         + cmd_end
//         + 1;

//     let flags_end = single_byte
//         .position(|w| w == b" ")
//         .ok_or(ParseError::Incomplete)?
//         + key_end
//         + 1;
//     let flags_str =
//         std::str::from_utf8(&buf[(key_end + 1)..flags_end]).map_err(|_| ParseError::Invalid)?;
//     let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

//     let expiry_end = single_byte
//         .position(|w| w == b" ")
//         .ok_or(ParseError::Incomplete)?
//         + flags_end
//         + 1;
//     let expiry_str =
//         std::str::from_utf8(&buf[(flags_end + 1)..expiry_end]).map_err(|_| ParseError::Invalid)?;
//     let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

//     let bytes_end = single_byte
//         .position(|w| w == b" ")
//         .ok_or(ParseError::Incomplete)?
//         + expiry_end
//         + 1;
//     let bytes_str =
//         std::str::from_utf8(&buf[(expiry_end + 1)..bytes_end]).map_err(|_| ParseError::Invalid)?;
//     let bytes = bytes_str
//         .parse::<usize>()
//         .map_err(|_| ParseError::Invalid)?;

//     // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
//     let mut double_byte_windows = buf.windows(CRLF.len());
//     let mut noreply = false;

//     // get the position of the next space and first CRLF
//     let next_space = single_byte
//         .position(|w| w == b" ")
//         .map(|v| v + expiry_end + 1);
//     let first_crlf = double_byte_windows
//         .position(|w| w == CRLF.as_bytes())
//         .ok_or(ParseError::Incomplete)?;

//     let cas_end = if let Some(next_space) = next_space {
//         // if we have both, bytes_end is before the earlier of the two
//         if next_space < first_crlf {
//             // validate that noreply isn't malformed
//             if &buf[(next_space + 1)..(first_crlf)] == NOREPLY.as_bytes() {
//                 noreply = true;
//                 next_space
//             } else {
//                 return Err(ParseError::Invalid);
//             }
//         } else {
//             first_crlf
//         }
//     } else {
//         first_crlf
//     };

//     if (bytes_end + 1) >= cas_end {
//         return Err(ParseError::Invalid);
//     }

//     if let Ok(Ok(cas)) =
//         std::str::from_utf8(&buf[(bytes_end + 1)..cas_end]).map(|v| v.parse::<u64>())
//     {
//         let consumed = first_crlf + CRLF.len() + bytes + CRLF.len();
//         if buf.len() >= consumed {
//             // let buffer = buffer.split_to(consumed);
//             let request = MemcacheRequest {
//                 command: MemcacheCommand::Cas,
//                 keys: vec![buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice()]
//                     .into_boxed_slice(),
//                 flags,
//                 expiry,
//                 noreply,
//                 cas,
//                 value: Some(
//                     buffer[(first_crlf + CRLF.len())..(first_crlf + CRLF.len() + bytes)]
//                         .to_vec()
//                         .into_boxed_slice(),
//                 ),
//             };
//             buffer.advance(consumed);
//             Ok(request)
//         } else {
//             // buffer doesn't have all the bytes for the value yet
//             Err(ParseError::Incomplete)
//         }
//     } else {
//         // could not parse the cas value
//         Err(ParseError::Invalid)
//     }
// }

// fn parse_delete(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
//     let buf: &[u8] = (*buffer).borrow();

//     let mut single_byte = buf.windows(1);
//     // we already checked for this in the MemcacheParser::parse()
//     let cmd_end = single_byte.position(|w| w == b" ").unwrap();

//     let mut noreply = false;
//     let mut double_byte = buf.windows(CRLF.len());
//     // get the position of the next space and first CRLF
//     let next_space = single_byte.position(|w| w == b" ").map(|v| v + cmd_end + 1);
//     let first_crlf = double_byte
//         .position(|w| w == CRLF.as_bytes())
//         .ok_or(ParseError::Incomplete)?;

//     let key_end = if let Some(next_space) = next_space {
//         // if we have both, bytes_end is before the earlier of the two
//         if next_space < first_crlf {
//             // validate that noreply isn't malformed
//             if &buf[(next_space + 1)..(first_crlf)] == NOREPLY.as_bytes() {
//                 noreply = true;
//                 next_space
//             } else {
//                 return Err(ParseError::Invalid);
//             }
//         } else {
//             first_crlf
//         }
//     } else {
//         first_crlf
//     };

//     let consumed = if noreply {
//         key_end + NOREPLY.len() + CRLF.len()
//     } else {
//         key_end + CRLF.len()
//     };

//     let request = MemcacheRequest {
//         command: MemcacheCommand::Delete,
//         keys: vec![buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice()].into_boxed_slice(),
//         noreply,
//         cas: 0,
//         expiry: 0,
//         value: None,
//         flags: 0,
//     };

//     buffer.advance(consumed);

//     Ok(request)
// }

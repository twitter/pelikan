// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use bytes::*;
use std::borrow::Borrow;
use std::io::*;

pub struct Buffer {
    rx: BytesMut,
    tx: BytesMut,
    tmp: Vec<u8>,
}

impl Buffer {
    pub fn new(rx: usize, tx: usize) -> Buffer {
        Buffer {
            rx: BytesMut::with_capacity(rx),
            tx: BytesMut::with_capacity(tx),
            tmp: vec![0; rx],
        }
    }

    pub fn clear(&mut self) {
        self.rx.clear();
        self.tx.clear();
    }

    // write from the tx buffer to a given sink
    pub fn write_to<T: Write>(&mut self, sink: &mut T) -> Result<Option<usize>> {
        match sink.write(self.tx.bytes()) {
            Ok(bytes) => {
                self.tx.advance(bytes);
                Ok(Some(bytes))
            }
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    // read from the source into the rx buffer
    pub fn read_from<T: Read>(&mut self, source: &mut T) -> Result<Option<usize>> {
        match source.read(&mut self.tmp) {
            Ok(bytes) => {
                self.rx.put(&self.tmp[0..bytes]);
                Ok(Some(bytes))
            }
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn tx_pending(&self) -> usize {
        self.tx.len()
    }

    pub fn rx_buffer(&self) -> &[u8] {
        self.rx.borrow()
    }
}

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.tx.put_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Read for Buffer {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut buffer: Cursor<&[u8]> = Cursor::new(self.rx.borrow());
        buffer.read(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_and_write_to() {
        let mut buffer = Buffer::new(4096, 4096);

        let messages: Vec<&[u8]> = vec![
            b"break the ice",
            b"he hath eaten me out of house and home",
            b"brevity is the soul of wit",
        ];

        for message in messages {
            buffer.write(message).expect("write failed");
            let mut sink = Vec::new();
            if let Ok(Some(len)) = buffer.write_to(&mut sink) {
                assert_eq!(sink.len(), len);
                assert_eq!(sink, message);
            }
        }
    }

    #[test]
    fn read_from_and_read() {
        let mut buffer = Buffer::new(4096, 4096);

        let mut messages: Vec<&[u8]> = vec![
            b"break the ice",
            b"he hath eaten me out of house and home",
            b"brevity is the soul of wit",
        ];

        for message in &mut messages {
            let check = message.clone();
            buffer.read_from(message).expect("read from failed");
            let mut sink = vec![0; 4096];
            if let Ok(len) = buffer.read(&mut sink) {
                sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
                assert_eq!(sink.len(), len); // sink contains the number of bytes read into it
                assert_eq!(sink.len(), check.len()); // sink has same number of bytes as check string
                assert_eq!(sink.as_slice(), check); // sink has same data as check string
                buffer.clear();
            }
        }
    }

    #[test]
    fn read_from_multiple() {
        let mut buffer = Buffer::new(4096, 4096);

        let mut messages: Vec<&[u8]> = vec![b"brave ", b"new ", b"world"];

        for message in &mut messages {
            buffer.read_from(message).expect("read from failed");
        }

        let check = b"brave new world";
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), len); // sink contains the number of bytes read into it
            assert_eq!(sink.len(), check.len()); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), check); // sink has same data as check string
            buffer.clear();
        }
        sink.clear();
        if let Ok(len) = buffer.read(&mut sink) {
            assert_eq!(len, 0);
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), len); // sink contains the number of bytes read into it
            assert_eq!(sink.as_slice(), b""); // sink has same data as check string
            buffer.clear();
        }
    }

    #[test]
    fn write_to_multiple() {
        let mut buffer = Buffer::new(4096, 4096);

        buffer.write(b"DEAD").expect("write failed");

        let check = b"DEAD";

        let mut sink = Vec::new();
        assert_eq!(buffer.write_to(&mut sink).unwrap().unwrap(), 4);
        assert_eq!(sink.len(), check.len());
        assert_eq!(sink.as_slice(), check);

        buffer.write(b"BEEF").expect("write failed");

        let check = b"BEEF";

        let mut sink = Vec::new();
        assert_eq!(buffer.write_to(&mut sink).unwrap().unwrap(), 4);
        assert_eq!(sink.len(), check.len());
        assert_eq!(sink.as_slice(), check);

        buffer.write(b"DEAD").expect("write failed");
        buffer.write(b"BEEF").expect("write failed");

        let check = b"DEADBEEF";

        let mut sink = Vec::new();
        assert_eq!(buffer.write_to(&mut sink).unwrap().unwrap(), 8);
        assert_eq!(sink.len(), check.len());
        assert_eq!(sink.as_slice(), check);
    }

    #[test]
    fn partial_read() {
        let mut buffer = Buffer::new(4096, 4096);

        let mut messages: Vec<&[u8]> = vec![b"DEAD", b"BEEF"];

        buffer
            .read_from(&mut messages[0])
            .expect("read from failed");

        // reading first time should have the contents we just wrote
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), 4); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), b"DEAD"); // sink has same data as check string
        }

        // reading again gives us the same result
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), 4); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), b"DEAD"); // sink has same data as check string
        }

        // append to the buffer and read to get all written data
        buffer
            .read_from(&mut messages[1])
            .expect("read from failed");
        let mut sink = vec![0; 4096];
        if let Ok(len) = buffer.read(&mut sink) {
            sink.truncate(len); // we need to shrink our buffer to match the bytes we've read
            assert_eq!(sink.len(), 8); // sink has same number of bytes as check string
            assert_eq!(sink.as_slice(), b"DEADBEEF"); // sink has same data as check string
        }
    }
}

use bytes::{Buf, Bytes, BytesMut};
use memmap::{Mmap, MmapOptions};
use std::cell::RefCell;
use std::fs::File;
use std::io::{Cursor, Read};
use std::ops::Deref;
use std::os::unix::fs::FileExt;
use std::sync::Arc;
use super::Result;

#[derive(Debug)]
#[repr(C, u8)]
pub enum SliceFactory {
    HeapStorage(HeapWrap),
    MmapStorage(MMapWrap),
    StdioStorage(FileWrap),
}

#[derive(Debug)]
#[repr(C)]
pub struct HeapWrap(Bytes);

impl HeapWrap {
    pub fn load(path: &str) -> Result<HeapWrap> {
        let mut f = File::open(path)?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        Ok(HeapWrap(Bytes::from(buffer)))
    }

    pub fn slice(&self, start: usize, end: usize) -> Result<Bytes> {
        assert!(end >= start);

        if end == start {
            return Ok(Bytes::new());
        }

        Ok(Bytes::from(&self.0[start..end]))
    }
}

impl Deref for HeapWrap {
    type Target = Bytes;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}

impl Clone for HeapWrap {
    fn clone(&self) -> Self {
        HeapWrap(self.0.clone())
    }
}

const BUF_LEN: usize = 8192;

pub fn readybuf(size: usize) -> BytesMut {
    let mut b = BytesMut::with_capacity(size);
    b.resize(size, 0);
    b
}

impl SliceFactory {
    pub fn load(path: &str) -> Result<SliceFactory> {
        let mut f = File::open(path)?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        Ok(SliceFactory::HeapStorage(HeapWrap(Bytes::from(buffer))))
    }

    pub fn make_map(path: &str) -> Result<SliceFactory> {
        let f = File::open(path)?;
        let mmap: Mmap = unsafe { MmapOptions::new().map(&f)? };

        let mut buf = [0u8; BUF_LEN];
        let mut count = 0;

        debug!("begin pretouch pages");
        {
            let mut cur = Cursor::new(&mmap[..]);
            loop {
                let remain = cur.remaining();
                if remain < BUF_LEN {
                    let mut buf = readybuf(remain);
                    cur.copy_to_slice(&mut buf[..]);
                    count += buf.len();
                    break;
                } else {
                    cur.copy_to_slice(&mut buf);
                    count += BUF_LEN;
                }
            }
        }
        debug!("end pretouch pages: {} bytes", count);

        Ok(SliceFactory::MmapStorage(MMapWrap::new(mmap)))
    }

    pub fn make_filewrap(path: &str) -> Result<SliceFactory> {
        Ok(SliceFactory::StdioStorage(FileWrap::open(path)?))
    }

    pub fn slice(&self, start: usize, end: usize) -> Result<Bytes> {
        assert!(end >= start);

        if end == start {
            return Ok(Bytes::new());
        }

        let range_len = end - start;

        match self {
            SliceFactory::HeapStorage(bytes) => Ok(Bytes::from(&bytes[start..end])),
            SliceFactory::MmapStorage(mmap) => {
                let mut v = Vec::with_capacity(range_len);
                v.extend_from_slice(&mmap[start..end]);
                Ok(Bytes::from(v))
            }
            SliceFactory::StdioStorage(filewrap) => filewrap.slice(start, end),
        }
    }
}

impl Clone for SliceFactory {
    fn clone(&self) -> Self {
        match self {
            SliceFactory::HeapStorage(bytes) => SliceFactory::HeapStorage(bytes.clone()),
            SliceFactory::MmapStorage(mmap) => SliceFactory::MmapStorage(mmap.clone()),
            SliceFactory::StdioStorage(fw) => SliceFactory::StdioStorage(fw.clone()),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MMapWrap {
    inner: Arc<Mmap>
}

impl MMapWrap {
    fn new(m: Mmap) -> MMapWrap {
        MMapWrap{inner: Arc::new(m)}
    }
}

impl Deref for MMapWrap {
    type Target = Mmap;

    fn deref(&self) -> &<Self as Deref>::Target {
        self.inner.as_ref()
    }
}

impl Clone for MMapWrap {
    fn clone(&self) -> Self {
        MMapWrap{inner: self.inner.clone()}
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FileWrap {
    inner: RefCell<File>,
    path: String,
}

impl FileWrap {
    fn new(f: File, path: &str) -> Self {
        FileWrap {
            inner: RefCell::new(f),
            path: path.to_string(),
        }
    }

    fn open(path: &str) -> Result<Self> {
        Ok(FileWrap::new(File::open(path)?, path))
    }

    fn slice(&self, start: usize, end: usize) -> Result<Bytes> {
        assert!(end >= start);
        let mut buf = readybuf(end - start);
        {
            let fp = self.inner.borrow_mut();
            fp.read_at(&mut buf, start as u64)?;
            trace!("read: {:?}", buf);
        }
        Ok(Bytes::from(buf))
    }

    #[cfg(test)]
    fn temp() -> Result<FileWrap> {
        use tempfile::*;

        let tmp = NamedTempFile::new()?;
        let fw = FileWrap::new(tmp.as_file().try_clone()?, tmp.path().to_str().unwrap());
        Ok(fw)
    }
}

impl Clone for FileWrap {
    fn clone(&self) -> Self {
        FileWrap::open(self.path.as_ref()).unwrap()
    }
}

#[repr(C)]
struct BMString(BytesMut);

impl ToString for BMString {
    fn to_string(&self) -> String {
       String::from(self)
    }
}

impl<'a> From<&'a BMString> for String {
    fn from(bm: &'a BMString) -> Self {
        String::from_utf8(bm.0.to_vec()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::prelude::*;
    use super::*;
    use tempfile;

    fn assert_ok<T>(f: T)
    where
        T: Fn() -> Result<()>,
    {
        f().unwrap()
    }

    #[test]
    fn basic_file_io_sanity() {
        assert_ok(|| {
            let mut tmp: File = tempfile::tempfile()?;

            tmp.write_all("abcdefghijklmnopqrstuvwxyz".as_bytes())?;
            tmp.sync_all()?;

            let mut buf = BytesMut::with_capacity(3);
            buf.resize(3, 0);
            let n = tmp.read_at(&mut buf, 23)?;
            assert_eq!(n, 3);
            assert_eq!(&buf[..], "xyz".as_bytes());
            Ok(())
        })
    }

    #[test]
    fn file_wrap_slice_test() {
        assert_ok(||{
            let fw = FileWrap::temp()?;

            {
                let mut f = fw.inner.borrow_mut();
                f.write_all("abcdefghijklmnopqrstuvwxyz".as_bytes())?;
                f.sync_all()?;
            }

            assert_eq!(fw.slice(3, 5)?, "de".as_bytes());
            Ok(())
        })
    }
}

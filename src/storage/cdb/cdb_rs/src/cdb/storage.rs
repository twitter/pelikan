use super::Result;
use bytes::{Buf, Bytes, BytesMut};
use memmap::{Mmap, MmapOptions};
use std::cell::RefCell;
use std::fs::File;
use std::io::{Cursor, Read};
use std::ops::{Deref, Range};
use std::os::unix::fs::FileExt;
use std::sync::Arc;

pub trait Sliceable {
    fn slice(&self, r: Range<usize>) -> Result<Bytes>;
}

const BUF_LEN: usize = 8192;

pub fn readybuf(size: usize) -> BytesMut {
    let mut b = BytesMut::with_capacity(size);
    b.resize(size, 0);
    b
}

impl Sliceable {
    pub fn load(path: &str) -> Result<HeapWrap> {
        let mut f = File::open(path)?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        Ok(HeapWrap(Bytes::from(buffer)))
    }

    pub fn make_map(path: &str) -> Result<MMapWrap> {
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

        Ok(MMapWrap::new(mmap))
    }

    pub fn make_filewrap(path: &str) -> Result<FileWrap> {
        Ok(FileWrap::open(path)?)
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct HeapWrap(Bytes);

impl Sliceable for HeapWrap {
    fn slice(&self, r: Range<usize>) -> Result<Bytes> {
        let Range { start, end } = r;

        if (end - 1) == start {
            return Ok(Bytes::new());
        }

        // TODO(simms): yes this is a heap copy. change to zero-copy once we understand how to
        // integrate that
        let mut v = Vec::with_capacity(end - start);
        v.extend_from_slice(&self.0[start..end]);
        Ok(Bytes::from(v))
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MMapWrap {
    inner: Arc<Mmap>,
}

impl MMapWrap {
    fn new(m: Mmap) -> MMapWrap {
        MMapWrap { inner: Arc::new(m) }
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
        MMapWrap {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FileWrap {
    inner: RefCell<File>,
    path: String,
}

impl Sliceable for FileWrap {
    fn slice(&self, r: Range<usize>) -> Result<Bytes> {
        let Range { start, end } = r;

        if (end - 1) == start {
            return Ok(Bytes::new());
        }

        self.slice(start, end)
    }
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
    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
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
        assert_ok(|| {
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

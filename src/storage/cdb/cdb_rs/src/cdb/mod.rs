pub use self::backend::Backend;
pub use self::errors::CDBError;
use self::ffi::gen;
use bytes::{Buf, Bytes, IntoBuf};
use bytes::{BufMut, BytesMut};
use cc_binding as bind;
use ccommon_rs::bstring::BString;
use failure;
use std::cmp;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::io::{Seek, SeekFrom, Write};
use std::ops::Deref;
use std::path::Path;
use std::result;

pub mod backend;
pub mod errors;
pub mod ffi;
pub mod input;
pub mod storage;

pub const STARTING_HASH: u32 = 5381;
const MAIN_TABLE_SIZE: usize = 256;
const MAIN_TABLE_SIZE_BYTES: u32 = 2048;
const END_TABLE_ENTRY_SIZE: u32 = 8;
const DATA_HEADER_SIZE: u32 = 8;
const INDEX_ENTRY_SIZE: usize = 8;

pub type Result<T> = result::Result<T, failure::Error>;

// idea from https://raw.githubusercontent.com/jothan/cordoba/master/src/lib.rs
#[derive(Copy, Clone, Eq, PartialEq, Default)]
struct CDBHash(u32);

impl CDBHash {
    fn new(bytes: &[u8]) -> Self {
        let mut h = STARTING_HASH;

        for b in bytes {
            // wrapping here is explicitly for allowing overflow semantics:
            //
            //   Operations like + on u32 values is intended to never overflow,
            //   and in some debug configurations overflow is detected and
            //   results in a panic. While most arithmetic falls into this
            //   category, some code explicitly expects and relies upon
            //   modular arithmetic (e.g., hashing)
            //
            h = h.wrapping_shl(5).wrapping_add(h) ^ u32::from(*b)
        }
        CDBHash(h)
    }

    #[inline]
    fn table(self) -> usize {
        self.0 as usize % MAIN_TABLE_SIZE
    }

    #[inline]
    fn slot(self, num_ents: usize) -> usize {
        (self.0 as usize >> 8) % num_ents
    }

    #[inline]
    fn inner(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for CDBHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CDBHash(0x{:08x})", self.0)
    }
}

impl<'a> From<&'a CDBHash> for usize {
    #[inline]
    fn from(h: &'a CDBHash) -> Self {
        h.0 as usize
    }
}

impl<'a> From<&'a CDBHash> for u32 {
    #[inline]
    fn from(h: &'a CDBHash) -> Self {
        h.0
    }
}

#[derive(Copy, Clone)]
struct Bucket {
    ptr: u32,
    num_ents: u32,
}

impl fmt::Debug for Bucket {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        write!(
            f,
            "TableRec {{ ptr: {:>#010x}, num_ents: {:>#010x} }}",
            self.ptr, self.num_ents
        )
    }
}

impl Bucket {
    // returns the offset into the db of entry n of this bucket.
    // panics if n >= num_ents
    #[inline]
    fn entry_n_pos(self, n: u32) -> IndexEntryPos {
        assert!(n < self.num_ents);
        IndexEntryPos(self.ptr + (n * END_TABLE_ENTRY_SIZE))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct IndexEntryPos(u32);

impl From<IndexEntryPos> for u32 {
    #[inline]
    fn from(n: IndexEntryPos) -> Self {
        n.0
    }
}

impl From<IndexEntryPos> for usize {
    #[inline]
    fn from(n: IndexEntryPos) -> Self {
        n.0 as usize
    }
}

#[derive(Clone, Debug)]
pub struct KV {
    pub k: Bytes,
    pub v: Bytes,
}

#[derive(Clone, Debug)]
pub struct KVRef<'a> {
    pub k: &'a [u8],
    pub v: &'a [u8],
}

#[derive(Copy, Clone, Default)]
struct IndexEntry {
    hash: CDBHash,
    // the hash of the stored key
    ptr: u32, // pointer to the absolute position of the data in the db
}

enum LoadMethod {
    HEAP = 1,
    MMAP = 2,
}

pub struct CDBHandleConfig {
    bpath: BString,
    load_method: LoadMethod,
}

impl CDBHandleConfig {
    pub unsafe fn from_raw(ptr: *const gen::cdb_handle_create_config) -> CDBHandleConfig {
        assert!(!ptr.is_null());

        let cfg = &*ptr;
        // this looks nasty, the only reason we have to do this is that rust
        // sees the type of gen::cdb_handle_create_config.path as a cdb::ffi::gen::bstring
        // which is the same as cc_binding::bstring, so we just coerce one into the other
        // to make the compiler happy
        let bpath = BString::from_raw(&mut *(cfg.path) as *mut _ as *mut bind::bstring);

        let load_method = match cfg.load_method {
            gen::cdb_load_method_CDB_HEAP => LoadMethod::HEAP,
            gen::cdb_load_method_CDB_MMAP => LoadMethod::MMAP,
            _ => unreachable!(),
        };

        CDBHandleConfig { bpath, load_method }
    }

    pub fn path(&self) -> Result<&Path> {
        ::std::str::from_utf8(&*self.bpath)
            .map(Path::new)
            .map_err(|e| e.into())
    }

    pub fn into_cdb_handle(self) -> Result<cdb_handle> {
        match self.load_method {
            LoadMethod::HEAP => self
                .path()
                .and_then(backend::Backend::load_path)
                .map(cdb_handle::from),
            LoadMethod::MMAP => self
                .path()
                .and_then(backend::Backend::mmap_path)
                .map(cdb_handle::from),
        }
    }
}

// this struct crosses the FFI boundary (as a pointer), so we give it
// a snake_case name so it matches the style convention on the C side.
#[allow(non_camel_case_types)]
pub struct cdb_handle(Backend);

impl cdb_handle {
    pub unsafe fn from_raw<'a>(ptr: *mut cdb_handle) -> &'a cdb_handle {
        &*(ptr as *mut _)
    }

    pub fn new(b: Box<[u8]>) -> cdb_handle {
        cdb_handle(Backend::Heap(b))
    }
}

impl From<Backend> for cdb_handle {
    fn from(be: Backend) -> Self {
        cdb_handle(be)
    }
}

impl<'a> From<&'a cdb_handle> for Reader<'a> {
    fn from(h: &'a cdb_handle) -> Self {
        Reader(&h.0[..])
    }
}

#[derive(Debug)]
pub struct Reader<'a>(&'a [u8]);

pub fn load_bytes_at_path(path: &str) -> Result<Box<[u8]>> {
    let mut f = File::open(path)?;
    let mut buffer = Vec::with_capacity(f.metadata()?.len() as usize);
    f.read_to_end(&mut buffer)?;
    Ok(buffer.into_boxed_slice())
}

impl<'a> Reader<'a> {
    pub fn new<T: AsRef<[u8]>>(r: &'a T) -> Reader<'a> {
        Reader(r.as_ref())
    }

    // TODO: perform basic sanity checks of data (gee i wish there was a checksum
    // somwhere in this data format HINT HINT)
    //
    // * does the secondary index fall safely in the correct index range?
    // * do data pointers point at the data segment?

    #[inline]
    fn bucket_at(&self, idx: usize) -> Result<Bucket> {
        assert!(idx < MAIN_TABLE_SIZE);

        let off = 8 * idx;

        let slice = self[off..(off + 8)].as_ref();
        let mut b = slice.into_buf();
        assert_eq!(slice.len(), 8);
        trace!("bucket_at idx: {}, got buf: {:?}", idx, b);

        let ptr = b.get_u32_le();
        let num_ents = b.get_u32_le();

        Ok(Bucket { ptr, num_ents })
    }

    // returns the index entry at absolute position 'pos' in the db
    #[inline]
    fn index_entry_at(&self, pos: IndexEntryPos) -> Result<IndexEntry> {
        let pos: usize = pos.into();

        if (pos as u32) < MAIN_TABLE_SIZE_BYTES {
            panic!("position {:?} was in the main table!", pos)
        }

        let mut b = self[pos..(pos + 8)].into_buf();
        let hash = CDBHash(b.get_u32_le());
        let ptr = b.get_u32_le();

        Ok(IndexEntry { hash, ptr })
    }

    #[inline]
    fn get_kv_ref(&'a self, ie: IndexEntry) -> Result<KVRef<'a>> {
        let p = ie.ptr as usize;
        let b = self[p..(p + DATA_HEADER_SIZE as usize)].as_ref();

        let ksize = b[..4].into_buf().get_u32_le() as usize;
        let vsize = b[4..].into_buf().get_u32_le() as usize;

        let kstart = p + DATA_HEADER_SIZE as usize;
        let vstart = (kstart + ksize) as usize;

        let k = &self[kstart..(kstart + ksize)];
        let v = &self[vstart..(vstart + vsize)];

        Ok(KVRef { k, v })
    }

    pub fn get(&self, key: &[u8], buf: &mut [u8]) -> Result<Option<usize>> {
        let hash = CDBHash::new(key);
        let bucket = self.bucket_at(hash.table())?;

        if bucket.num_ents == 0 {
            trace!("bucket empty, returning none");
            return Ok(None);
        }

        let slot = hash.slot(bucket.num_ents as usize);

        for x in 0..bucket.num_ents {
            let index_entry_pos = bucket.entry_n_pos((x + slot as u32) % bucket.num_ents);

            let idx_ent = self.index_entry_at(index_entry_pos)?;

            if idx_ent.ptr == 0 {
                return Ok(None);
            } else if idx_ent.hash == hash {
                let kv = self.get_kv_ref(idx_ent)?;
                if &kv.k[..] == key {
                    return Ok(Some(copy_slice(buf, kv.v)));
                } else {
                    continue;
                }
            }
        }

        Ok(None)
    }
}

impl<'a> Deref for Reader<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.0
    }
}

#[inline]
fn copy_slice(dst: &mut [u8], src: &[u8]) -> usize {
    let n = cmp::min(dst.len(), src.len());
    dst[0..n].copy_from_slice(&src[0..n]);
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn kvs() -> Vec<(String, String)> {
        vec![
            ("abc", "def"),
            ("pink", "red"),
            ("apple", "grape"),
            ("q", "burp"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    }

    fn validate(pairs: &Vec<(String, String)>, cdb: &Reader) -> Result<()> {
        for (k, v) in pairs {
            let mut buf = Vec::new();
            buf.resize(10, 0u8);

            let n = cdb.get(k.as_bytes(), &mut buf[..]).unwrap().unwrap();
            assert_eq!(n, v.len());
            assert_eq!(&buf[0..n], v.as_bytes())
        }

        let mut buf = Vec::new();
        buf.resize(10, 0u8);

        let r = cdb.get("1233".as_bytes(), &mut buf[..]).unwrap();
        assert!(r.is_none());
        Ok(())
    }

    fn load_and_validate_cdb<'a, F, T>(kvs: &Vec<(String, String)>, f: F) -> Result<()>
    where
        F: FnOnce(&mut NamedTempFile) -> Result<T>,
        T: AsRef<[u8]>,
    {
        let mut ntf = NamedTempFile::new()?;

        {
            let mut w = Writer::new(ntf.as_file_mut())?;
            for kv in kvs {
                let (k, v) = kv.clone();
                w.put(&k.into_bytes(), &v.into_bytes())?;
            }
        }

        let data = f(&mut ntf)?;
        let cdb = Reader::new(&data);
        validate(&kvs, &cdb)
    }

    #[test]
    fn round_trip_boxed_slice_test() {
        let pairs = kvs();

        load_and_validate_cdb(&pairs, |ntf| {
            let mut buf = Vec::new();
            ntf.read_to_end(&mut buf)?;
            Ok(buf.into_boxed_slice())
        })
        .unwrap()
    }

    #[test]
    fn round_trip_heap_backend() {
        let pairs = kvs();

        load_and_validate_cdb(&pairs, |ntf| Backend::load(ntf.as_file_mut())).unwrap()
    }

    #[test]
    fn round_trip_mmap_backend() {
        let pairs = kvs();

        load_and_validate_cdb(&pairs, |ntf| Backend::mmap(ntf.as_file_mut())).unwrap()
    }
}

fn ready_buf(size: usize) -> BytesMut {
    let mut b = BytesMut::with_capacity(size);
    b.reserve(size);
    b
}

pub struct Writer<'a, F>
where
    F: Write + Seek + 'a,
{
    file: &'a mut F,
    index: Vec<Vec<IndexEntry>>,
}

impl<'a, F> Writer<'a, F>
where
    F: Write + Seek + 'a,
{
    pub fn new(file: &'a mut F) -> Result<Writer<'a, F>> {
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&[0u8; MAIN_TABLE_SIZE_BYTES as usize])?;

        Ok(Writer {
            file,
            index: vec![vec![IndexEntry::default()]; 256],
        })
    }

    fn seek(&mut self, sf: SeekFrom) -> Result<u32> {
        self.file.seek(sf).map(|n| n as u32).map_err(|e| e.into())
    }

    pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let ptr = self.seek(SeekFrom::Current(0))?;
        let mut buf = ready_buf(INDEX_ENTRY_SIZE + key.len() + value.len());

        buf.put_u32_le(key.len() as u32);
        buf.put_u32_le(value.len() as u32);
        buf.extend_from_slice(key);
        buf.extend_from_slice(value);

        self.file.write_all(&buf[..])?;

        let hash = CDBHash::new(key);
        self.index[hash.table() as usize].push(IndexEntry { hash, ptr });
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        let mut buckets: Vec<Bucket> = Vec::with_capacity(256);
        self.file.seek(SeekFrom::End(0))?;

        let idx = self.index.clone();

        for tbl in idx {
            let length = (tbl.len() << 1) as u32;
            let mut ordered: Vec<IndexEntry> = vec![IndexEntry::default(); length as usize];
            for idx_ent in tbl {
                let slot = idx_ent.hash.slot(length as usize);
                for i in 0..length {
                    let j = (i + slot as u32) % length;
                    if ordered[j as usize].ptr == 0 {
                        ordered[j as usize] = idx_ent;
                        break;
                    }
                }
            }

            // move to EOF and write out the secondary index entries, constructing the
            // primary table as we go ('buckets')
            //
            buckets.push(Bucket {
                ptr: self.seek(SeekFrom::End(0))?,
                num_ents: length,
            });

            let mut buf = ready_buf((ordered.len() * 8) as usize);

            for idx_ent in ordered {
                buf.put_u32_le(idx_ent.hash.inner());
                buf.put_u32_le(idx_ent.ptr);
            }

            self.file.write_all(&buf[..])?;
        }

        // now write the buckets
        //
        self.file.seek(SeekFrom::Start(0))?;
        {
            let mut buf = ready_buf(2048);

            for bkt in buckets {
                buf.put_u32_le(bkt.ptr);
                buf.put_u32_le(bkt.num_ents);
            }

            self.file.write_all(&buf[..])?;
        }

        // start at BOF
        self.file.seek(SeekFrom::Start(0))?;

        Ok(())
    }
}

impl<'a, F> Drop for Writer<'a, F>
where
    F: Write + Seek + 'a,
{
    fn drop(&mut self) {
        self.finalize().unwrap();
    }
}

pub use self::errors::CDBError;

use bytes::{Buf, Bytes, IntoBuf};
use std::cmp;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::result;

pub mod errors;
pub mod input;
pub mod storage;

pub const STARTING_HASH: u32 = 5381;
const MAIN_TABLE_SIZE: usize = 256;
const MAIN_TABLE_SIZE_BYTES: usize = 2048;
const END_TABLE_ENTRY_SIZE: usize = 8;
const DATA_HEADER_SIZE: usize = 8;

pub type Result<T> = result::Result<T, CDBError>;

// idea from https://raw.githubusercontent.com/jothan/cordoba/master/src/lib.rs
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(C)]
struct CDBHash(u32);

impl CDBHash {
    fn new(bytes: &[u8]) -> Self {
        let mut h = STARTING_HASH;

        for b in bytes {
            // wrapping here is explicitly for allowing overflow semantics:
            //
            //   Operations like + on u32 values is intended to never overflow,
            //   and in some debug configurations overflow is detected and results in a panic.
            //   While most arithmetic falls into this category, some code explicitly expects
            //   and relies upon modular arithmetic (e.g., hashing)
            //
            h = h.wrapping_shl(5).wrapping_add(h) ^ u32::from(*b)
        }
        CDBHash(h)
    }

    fn table(self) -> usize {
        self.0 as usize % MAIN_TABLE_SIZE
    }

    fn slot(self, num_ents: usize) -> usize {
        (self.0 as usize >> 8) % num_ents
    }
}

impl fmt::Debug for CDBHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CDBHash(0x{:08x})", self.0)
    }
}

impl<'a> From<&'a CDBHash> for usize {
    fn from(h: &'a CDBHash) -> Self {
        h.0 as usize
    }
}

impl<'a> From<&'a CDBHash> for u32 {
    fn from(h: &'a CDBHash) -> Self {
        h.0
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
struct Bucket {
    ptr: usize,
    num_ents: usize,
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
    fn entry_n_pos(&self, n: usize) -> IndexEntryPos {
        assert!(n < self.num_ents);
        IndexEntryPos(self.ptr + (n * END_TABLE_ENTRY_SIZE))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
struct IndexEntryPos(usize);

impl From<IndexEntryPos> for usize {
    fn from(n: IndexEntryPos) -> Self {
        n.0
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct KV {
    pub k: Bytes,
    pub v: Bytes,
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct KVRef<'a> {
    pub k: &'a [u8],
    pub v: &'a [u8],
}

#[repr(C)]
struct IndexEntry {
    hash: CDBHash, // the hash of the stored key
    ptr: usize,    // pointer to the absolute position of the data in the db
}

#[derive(Debug)]
#[repr(C)]
pub struct CDB<'a> {
    data: &'a [u8],
}

pub fn load_bytes_at_path(path: &str) -> Result<Box<[u8]>> {
    let mut f = File::open(path)?;
    let mut buffer = Vec::with_capacity(f.metadata()?.len() as usize);
    f.read_to_end(&mut buffer)?;
    Ok(buffer.into_boxed_slice())
}

impl<'a> CDB<'a> {
    pub fn new(b: &[u8]) -> CDB {
        CDB { data: b }
    }

    #[inline]
    fn bucket_at(&self, idx: usize) -> Result<Bucket> {
        assert!(idx < MAIN_TABLE_SIZE);

        let off = 8 * idx;

        let slice = self.data[off..(off + 8)].as_ref();
        let b = slice.into_buf();
        assert_eq!(slice.len(), 8);
        trace!("bucket_at idx: {}, got buf: {:?}", idx, b);

        let mut buf = b.into_buf();

        let ptr = buf.get_u32_le() as usize;
        let num_ents = buf.get_u32_le() as usize;

        Ok(Bucket { ptr, num_ents })
    }

    // returns the index entry at absolute position 'pos' in the db
    #[inline]
    fn index_entry_at(&self, pos: IndexEntryPos) -> Result<IndexEntry> {
        let pos: usize = pos.into();

        if pos < MAIN_TABLE_SIZE_BYTES {
            panic!("position {:?} was in the main table!", pos)
        }

        let mut b = self.data[pos..(pos + 8)].into_buf();
        let hash = CDBHash(b.get_u32_le());
        let ptr = b.get_u32_le() as usize;

        Ok(IndexEntry { hash, ptr })
    }

    #[inline]
    fn get_kv_ref(&self, ie: &IndexEntry) -> Result<KVRef<'a>> {
        let b = self.data[ie.ptr..(ie.ptr + DATA_HEADER_SIZE)].as_ref();

        let ksize = b[..4].into_buf().get_u32_le() as usize;
        let vsize = b[4..].into_buf().get_u32_le() as usize;

        let kstart = ie.ptr + DATA_HEADER_SIZE;
        let vstart = kstart + ksize;

        let k = &self.data[kstart..(kstart + ksize)];
        let v = &self.data[vstart..(vstart + vsize)];

        Ok(KVRef { k, v })
    }

    pub fn get(&self, key: &[u8], buf: &mut[u8]) -> Result<Option<usize>> {
        let hash = CDBHash::new(key);
        let bucket = self.bucket_at(hash.table())?;

        if bucket.num_ents == 0 {
            trace!("bucket empty, returning none");
            return Ok(None);
        }

        let slot = hash.slot(bucket.num_ents);

        for x in 0..bucket.num_ents {
            let index_entry_pos = bucket.entry_n_pos((x + slot) % bucket.num_ents);

            let idx_ent = self.index_entry_at(index_entry_pos)?;

            if idx_ent.ptr == 0 {
                return Ok(None);
            } else if idx_ent.hash == hash {
                let kv = self.get_kv_ref(&idx_ent)?;
                // TODO: this is incorrect handling of the buffer! shit!
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

#[inline]
fn copy_slice(dst: &mut [u8], src: &[u8]) -> usize {
    let n = cmp::min(dst.len(), src.len());
    dst[0..n].copy_from_slice(&src[0..n]);
    n
}

#[cfg(test)]
mod tests {
    use std::collections::hash_set;
    use std::fs::remove_file;
    use std::path::PathBuf;
    use super::*;
    use tempfile::NamedTempFile;
    use tinycdb::Cdb as TCDB;

    fn create_temp_cdb<'a>(kvs: &Vec<(String, String)>) -> Result<Box<[u8]>> {
        let path: PathBuf;

        {
            let ntf = NamedTempFile::new()?;
            remove_file(ntf.path())?;
            path = ntf.path().to_path_buf();
        }

        let mut dupcheck = hash_set::HashSet::new();

        TCDB::new(path.as_ref(), |c| {
            let ys = kvs.to_owned();
            for (k, v) in ys {
                let kk = k.clone();
                let vv = v.clone();

                if !dupcheck.contains(&k) {
                    dupcheck.insert(k);
                    c.add(kk.as_ref(), vv.as_ref()).unwrap();
                }
            }
        }).unwrap();

        load_bytes_at_path(path.to_str().unwrap())
    }


    #[test]
    fn round_trip_test() {
        let kvs: Vec<(String, String)> = vec![
            ("abc", "def"),
            ("pink", "red"),
            ("apple", "grape"),
            ("q", "burp"),
        ].iter()
            .map(|(k,v)| (k.to_string(), v.to_string()))
            .collect();

        let data = create_temp_cdb(&kvs).unwrap();

        let cdb = CDB { data: &data };

        for (k, v) in kvs {
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
    }
}

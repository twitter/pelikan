use bytes::{Buf, Bytes, IntoBuf};
use std::fmt;
use std::io;
use std::io::Write;
use std::result;

pub mod errors;
pub mod input;
pub mod writer;
pub mod storage;

use self::storage::SliceFactory;

pub use self::errors::CDBError;

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
            h = h.wrapping_shl(5).wrapping_add(h) ^ (*b as u32)
        }
        CDBHash(h)
    }

    fn table(&self) -> usize {
        self.0 as usize % MAIN_TABLE_SIZE
    }

    fn slot(&self, num_ents: usize) -> usize {
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
    fn entry_n_pos<'a>(&'a self, n: usize) -> IndexEntryPos {
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

impl KV {
    #[allow(dead_code)]
    fn dump(&self, w: &mut impl io::Write) -> io::Result<()> {
        write!(w, "+{},{}:", self.k.len(), self.v.len())?;
        w.write(self.k.as_ref())?;
        write!(w, "->")?;
        w.write(self.v.as_ref())?;
        write!(w, "\n")
    }
}


struct IndexEntry {
    hash: CDBHash, // the hash of the stored key
    ptr: usize,    // pointer to the absolute position of the data in the db
}

#[repr(C)]
pub struct KVIter {
    cdb: Box<CDB>,
    bkt_idx: usize,
    entry_n: usize,
    bkt: Bucket,
}

impl KVIter {
    fn new(cdb: Box<CDB>) -> Result<Self> {
        let bkt = cdb.bucket_at(0)?;
        Ok(KVIter{cdb, bkt_idx: 0, entry_n: 0, bkt})
    }
}

impl Iterator for KVIter {
    type Item = Result<KV>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.bkt_idx >= MAIN_TABLE_SIZE {
                return None
            }

            if self.entry_n >= self.bkt.num_ents {
                self.bkt_idx += 1;
                self.entry_n = 0;
                if self.bkt_idx < MAIN_TABLE_SIZE {
                    match self.cdb.bucket_at(self.bkt_idx) {
                        Ok(bkt) => self.bkt = bkt,
                        Err(err) => return Some(Err(err)),
                    }
                }
                continue
            }

            let idx_ent =
                match self.cdb.index_entry_at(self.bkt.entry_n_pos(self.entry_n)) {
                    Ok(index_entry) => index_entry,
                    Err(err) => return Some(Err(err)),
                };

            self.entry_n += 1;

            if idx_ent.ptr == 0 {
                continue
            } else {
                return match self.cdb.get_kv(idx_ent) {
                    Ok(kv) => Some(Ok(kv)),
                    Err(err) => Some(Err(err)),
                }
            }
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct CDB {
    data: SliceFactory,
}

impl Clone for CDB {
    fn clone(&self) -> Self {
        CDB{data: self.data.clone()}
    }
}

impl CDB {
    pub fn new(sf: SliceFactory) -> CDB { CDB{data: sf} }

    pub fn stdio(path: &str) -> Result<CDB> {
        Ok(CDB::new(SliceFactory::make_filewrap(path)?))
    }

    pub fn mmap(path: &str) -> Result<CDB> {
        Ok(CDB::new(SliceFactory::make_map(path)?))
    }

    pub fn load(path: &str) -> Result<CDB> {
        Ok(CDB::new(SliceFactory::load(path)?))
    }

    pub fn kvs_iter(&self) -> Result<KVIter> {
        Ok(KVIter::new(Box::new(self.clone()))?)
    }

    #[inline]
    fn bucket_at(&self, idx: usize) -> Result<Bucket> {
        assert!(idx < MAIN_TABLE_SIZE);

        let off = 8 * idx;

        let b = self.data.slice(off, off + 8)?;
        assert_eq!(b.len(), 8);
        trace!("bucket_at idx: {}, got buf: {:?}", idx, b);

        let mut buf = b.into_buf();

        let ptr = buf.get_u32_le() as usize;
        let num_ents = buf.get_u32_le() as usize;

        Ok(Bucket{ptr, num_ents})
    }

    // returns the index entry at absolute position 'pos' in the db
    #[inline]
    fn index_entry_at(&self, pos: IndexEntryPos) -> Result<IndexEntry> {
        let pos: usize = pos.into();

        if pos < MAIN_TABLE_SIZE_BYTES {
            panic!("position {:?} was in the main table!", pos)
        }

        let mut b = self.data.slice(pos, pos + 8)?.into_buf();
        let hash = CDBHash(b.get_u32_le());
        let ptr = b.get_u32_le() as usize;

        Ok(IndexEntry{hash, ptr})
    }

    #[inline]
    fn get_kv(&self, ie: IndexEntry) -> Result<KV> {
        let b = self.data.slice(ie.ptr, ie.ptr + DATA_HEADER_SIZE)?;

        let ksize = b.slice_to(4).into_buf().get_u32_le() as usize;
        let vsize = b.slice_from(4).into_buf().get_u32_le() as usize;

        let kstart = ie.ptr + DATA_HEADER_SIZE;
        let vstart = kstart + ksize;

        let k = self.data.slice(kstart, kstart + ksize)?;
        let v = self.data.slice(vstart, vstart + vsize)?;

        Ok(KV{k: Bytes::from(k), v: Bytes::from(v)})
    }

    pub fn get(&self, key: &[u8], buf: &mut Vec<u8>) -> Result<Option<usize>> {
        let key = key.into();
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
                let kv = self.get_kv(idx_ent)?;
                if &kv.k[..] == key {
                    buf.write_all(&kv.k[..]).unwrap();
                    return Ok(Some(kv.k.len()));
                } else {
                    continue;
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use env_logger;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::string;
    use std::collections::hash_set;
    use std::fs::File;
    use std::fs::remove_file;
    use std::io::{BufRead, BufReader};
    use std::path::Path;
    use std::path::PathBuf;
    use super::*;
    use tempfile::NamedTempFile;
    use tinycdb::Cdb as TCDB;

    fn arb_string_slice<'a>() -> BoxedStrategy<Vec<String>> {
        let st = string::string_regex("[a-z]+").unwrap();
        vec(st, 10..1000).boxed()
    }

    struct QueryResult(String, Option<String>);

    #[allow(dead_code)]
    fn create_temp_cdb<'a>(kvs: &Vec<(String, String)>) -> Result<SliceFactory> {
        let path: PathBuf;

        {
            let ntf = NamedTempFile::new()?;
            remove_file(ntf.path())?;
            path = ntf.path().to_owned();
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

        let sf = self::storage::SliceFactory::load(path.to_str().unwrap())?;

        Ok(sf)
    }

    proptest! {
        #[test]
        fn qc_key_and_value_retrieval(ref xs in arb_string_slice()) {
            let cdb = CDB::new(make_temp_cdb_single_vals(&xs));

            for QueryResult(q, r) in read_keys(&cdb, &xs) {
                prop_assert_eq!(
                    Some(q),
                    r
                );
            }
        }
    }

    type QueryResultIter<'a> = Box<Iterator<Item = QueryResult> + 'a>;

    fn read_keys<'a>(cdb: &'a CDB, xs: &'a Vec<String>) -> QueryResultIter<'a> {
        Box::new(xs.iter().map(move |x| {
            let mut buf = Vec::with_capacity(1024 * 1024);
            let res = cdb.get(x.as_ref(), &mut buf).unwrap();
            QueryResult(
                x.clone(),
                res.map(|_| String::from_utf8(buf).unwrap()),
            )
        }))
    }

    #[allow(dead_code)]
    fn make_temp_cdb_single_vals(xs: &Vec<String>) -> SliceFactory {
        let kvs: Vec<(String, String)> =
            xs.iter().map(|k| (k.to_owned(), k.to_owned())).collect();
        create_temp_cdb(&kvs).unwrap()
    }

    #[test]
    fn read_small_list() {
        env_logger::try_init().unwrap();

        let strings = vec![
            "shngcmfkqjtvhnbgfcvbm",
            "qjflpsvacyhsgxykbvarbvmxapufmdt",
            "a",
            "a",
            "a",
            "a",
            "a",
            "a",
            "xfjhaqjkcjiepmcbhopgpxwwth",
            "a",
            "a",
        ];
        let arg = strings.iter().map(|s| (*s).to_owned()).collect();

        let sf = make_temp_cdb_single_vals(&arg);
        let cdb = CDB::new(sf);

        for QueryResult(q, r) in read_keys(&cdb, &arg) {
            assert_eq!(Some(q), r);
        }
    }

    #[test]
    fn test_with_dictionary() {
        let mut args: Vec<String> = Vec::new();

        {
            let f = File::open(Path::new("/usr/share/dict/words")).unwrap();
            let bufr = BufReader::new(&f);

            for line in bufr.lines() {
                let word = line.unwrap();
                args.push(word.to_owned());
            }
        }

        let cdb = CDB::new(make_temp_cdb_single_vals(&args));

        for QueryResult(q, r) in read_keys(&cdb, &args) {
            assert_eq!(Some(q), r);
        }
    }
}

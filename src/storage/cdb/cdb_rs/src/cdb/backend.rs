use memmap;
use std::ops::Deref;
use super::Result;
use std::fs::File;
use std::io::Read;
use std::path::Path;


pub enum Backend {
    Heap(Box<[u8]>),
    MMap(memmap::Mmap),
}

impl Deref for Backend {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Backend::Heap(bx) => &bx[..],
            Backend::MMap(mm) => &mm[..],
        }
    }
}

impl AsRef<[u8]> for Backend {
    fn as_ref(&self) -> &[u8] { &self[..] }
}

impl Backend {
    pub fn load_path(p: &Path) -> Result<Backend> {
        let fp = File::open(p)?;
        Backend::load(&fp)
    }

    pub fn load(mut fp: &File) -> Result<Backend> {
        let mut buffer = Vec::new();
        fp.read_to_end(&mut buffer)?;
        Ok(Backend::Heap(buffer.into_boxed_slice()))
    }

    pub fn mmap_path(p: &Path) -> Result<Backend> {
        let fp = File::open(p)?;
        Backend::mmap(&fp)
    }

    pub fn mmap(fp: &File) -> Result<Backend> {
        Ok(Backend::MMap(
            unsafe { memmap::MmapOptions::new().map(&fp)? }
        ))
    }
}

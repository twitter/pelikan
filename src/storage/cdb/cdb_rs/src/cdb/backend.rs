// Copyright (C) 2018-2020 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::{Result, MAIN_TABLE_SIZE_BYTES};

use memmap;

use std::fs::File;
use std::io::{Cursor, Read};
use std::ops::Deref;
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
    fn as_ref(&self) -> &[u8] {
        &self[..]
    }
}

impl Backend {
    pub fn noop() -> Result<Backend> {
        let v = {
            let buf = Vec::with_capacity(MAIN_TABLE_SIZE_BYTES as usize);
            let mut cur = Cursor::new(buf);
            super::Writer::new(&mut cur)?;
            cur.into_inner()
        };

        Ok(Backend::Heap(v.into_boxed_slice()))
    }

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
        Ok(Backend::MMap(unsafe {
            memmap::MmapOptions::new().map(&fp)?
        }))
    }
}

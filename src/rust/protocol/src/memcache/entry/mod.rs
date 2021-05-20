pub struct MemcacheEntry {
    pub key: Box<[u8]>,
    pub value: Box<[u8]>,
    pub expiry: u32,
    pub flags: u32,
    pub cas: Option<u64>,
}

impl MemcacheEntry {
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub fn expiry(&self) -> u32 {
        self.expiry
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn cas(&self) -> Option<u64> {
        self.cas
    }
}
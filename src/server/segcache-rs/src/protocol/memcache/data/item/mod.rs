pub struct MemcacheItem {
    pub key: Box<[u8]>,
    pub value: Box<[u8]>,
    pub flags: u32,
    pub cas: Option<u32>,
}

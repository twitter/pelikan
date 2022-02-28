#[derive(Debug)]
pub struct MomentoCache {
    pub cache_name: String,
}

#[derive(Debug)]
pub struct MomentoListCacheResult {
    pub caches: Vec<MomentoCache>,
    pub next_token: String,
}

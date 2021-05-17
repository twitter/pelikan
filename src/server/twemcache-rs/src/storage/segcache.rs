use config::SegCacheConfig;
use crate::protocol::traits::Init;
use crate::protocol::traits::Response;
use crate::protocol::data::*;
use crate::protocol::traits::Execute;
use crate::*;
use config::segcache::Eviction;
use config::TimeType;
use metrics::*;
use rustcommon_time::CoarseDuration;
use segcache::*;
use std::time::SystemTime;

pub trait MemcacheStorage {
    fn get(&mut self, keys: &[&[u8]]) -> MemcacheResponse;
    fn gets(&mut self, keys: &[&[u8]]) -> MemcacheResponse;
    fn set(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;
    fn add(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;
    fn replace(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;
    fn delete(&mut self, key: &[u8], noreply: bool) -> MemcacheResponse;
    fn cas(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
        cas: u64,
    ) -> MemcacheResponse;
}

impl<T: MemcacheStorage> Execute<MemcacheRequest> for T {
    fn execute(&mut self, request: MemcacheRequest) -> Box<dyn Response> {
        match request.command() {
            MemcacheCommand::Get => {
                let keys: Vec<&[u8]> = request.keys().collect();
                Box::new(self.get(&keys))
            }
            MemcacheCommand::Gets => {
                let keys: Vec<&[u8]> = request.keys().collect();
                Box::new(self.gets(&keys))
            }
            MemcacheCommand::Set => {
                let key = request.keys().next().unwrap();
                Box::new(self.set(
                    key,
                    request.value(),
                    request.flags(),
                    request.expiry(),
                    request.noreply(),
                ))
            }
            MemcacheCommand::Add => {
                let key = request.keys().next().unwrap();
                Box::new(self.add(
                    key,
                    request.value(),
                    request.flags(),
                    request.expiry(),
                    request.noreply(),
                ))
            }
            MemcacheCommand::Replace => {
                let key = request.keys().next().unwrap();
                Box::new(self.replace(
                    key,
                    request.value(),
                    request.flags(),
                    request.expiry(),
                    request.noreply(),
                ))
            }
            MemcacheCommand::Delete => {
                let key = request.keys().next().unwrap();
                Box::new(self.delete(key, request.noreply()))
            }
            MemcacheCommand::Cas => {
                let key = request.keys().next().unwrap();
                Box::new(self.cas(
                    key,
                    request.value(),
                    request.flags(),
                    request.expiry(),
                    request.noreply(),
                    request.cas(),
                ))
            }
        }
    }
}

/// A wrapper type to construct storage and perform any config-sensitive
/// processing
pub struct SegCacheStorage {
    config: Arc<Config>,
    data: SegCache,
}

impl Init<Config> for SegCacheStorage {
    fn new(config: Arc<Config>) -> Self {
        Self::new(config.segcache())
    }
}

impl SegCacheStorage {
    pub(crate) fn new(config: &SegCacheConfig) -> Self {
        let eviction = match config.eviction() {
            Eviction::None => Policy::None,
            Eviction::Random => Policy::Random,
            Eviction::Fifo => Policy::Fifo,
            Eviction::Cte => Policy::Cte,
            Eviction::Util => Policy::Util,
            Eviction::Merge => Policy::Merge {
                max: config.merge_max(),
                merge: config.merge_target(),
                compact: config.compact_target(),
            },
        };

        let data = SegCache::builder()
            .power(config.hash_power())
            .overflow_factor(config.overflow_factor())
            .heap_size(config.heap_size())
            .segment_size(config.segment_size())
            .eviction(eviction)
            .datapool_path(config.datapool_path())
            .build();

        Self { config, data }
    }

    pub fn expire(&mut self) {
        self.data.expire();
    }

    /// converts the request expiry to a ttl
    fn get_ttl(&mut self, expiry: u32) -> u32 {
        match self.config.time().time_type() {
            TimeType::Unix => {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                expiry.wrapping_sub(epoch)
            }
            TimeType::Delta => expiry,
            TimeType::Memcache => {
                if expiry < 60 * 60 * 24 * 30 {
                    expiry
                } else {
                    let epoch = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as u32;
                    expiry.wrapping_sub(epoch)
                }
            }
        }
    }
}

impl MemcacheStorage for SegCacheStorage {
    fn get(&mut self, keys: &[&[u8]]) -> MemcacheResponse {
        increment_counter!(&Stat::Get);
        let mut items = Vec::new();
        for key in keys {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                items.push(MemcacheItem {
                    key: item.key().to_vec().into_boxed_slice(),
                    value: item.value().to_vec().into_boxed_slice(),
                    flags,
                    cas: None,
                });
            }
        }

        increment_counter_by!(&Stat::GetKey, keys.len() as u64);
        increment_counter_by!(&Stat::GetKeyHit, items.len() as u64);
        increment_counter_by!(&Stat::GetKeyMiss, keys.len() as u64 - items.len() as u64);

        MemcacheResponse::Items(items.into_boxed_slice())
    }

    fn gets(&mut self, keys: &[&[u8]]) -> MemcacheResponse {
        increment_counter!(&Stat::Get);
        let mut items = Vec::new();
        for key in keys {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                items.push(MemcacheItem {
                    key: item.key().to_vec().into_boxed_slice(),
                    value: item.value().to_vec().into_boxed_slice(),
                    flags,
                    cas: Some(item.cas()),
                });
            }
        }

        increment_counter_by!(&Stat::GetKey, keys.len() as u64);
        increment_counter_by!(&Stat::GetKeyHit, items.len() as u64);
        increment_counter_by!(&Stat::GetKeyMiss, keys.len() as u64 - items.len() as u64);

        MemcacheResponse::Items(items.into_boxed_slice())
    }

    fn set(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Set);
        let ttl = self.get_ttl(expiry);
        let value = value.unwrap_or(b"");
        match self.data.insert(
            key,
            value,
            Some(&flags.to_be_bytes()),
            CoarseDuration::from_secs(ttl),
        ) {
            Ok(_) => {
                increment_counter!(&Stat::SetStored);
                if noreply {
                    MemcacheResponse::NoReply
                } else {
                    MemcacheResponse::Stored
                }
            }
            Err(_) => {
                increment_counter!(&Stat::SetNotstored);
                if noreply {
                    MemcacheResponse::NoReply
                } else {
                    MemcacheResponse::NotStored
                }
            }
        }
    }

    fn add(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Add);
        let ttl = self.get_ttl(expiry);
        let value = value.unwrap_or(b"");
        if self.data.get_no_freq_incr(key).is_none()
            && self
                .data
                .insert(
                    key,
                    value,
                    Some(&flags.to_be_bytes()),
                    CoarseDuration::from_secs(ttl),
                )
                .is_ok()
        {
            increment_counter!(&Stat::AddStored);
            if noreply {
                MemcacheResponse::NoReply
            } else {
                MemcacheResponse::Stored
            }
        } else {
            increment_counter!(&Stat::AddNotstored);
            if noreply {
                MemcacheResponse::NoReply
            } else {
                MemcacheResponse::NotStored
            }
        }
    }

    fn replace(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Replace);
        let ttl = self.get_ttl(expiry);
        let value = value.unwrap_or(b"");
        if self.data.get_no_freq_incr(key).is_some()
            && self
                .data
                .insert(
                    key,
                    value,
                    Some(&flags.to_be_bytes()),
                    CoarseDuration::from_secs(ttl),
                )
                .is_ok()
        {
            increment_counter!(&Stat::ReplaceStored);
            if noreply {
                MemcacheResponse::NoReply
            } else {
                MemcacheResponse::Stored
            }
        } else {
            increment_counter!(&Stat::ReplaceNotstored);
            if noreply {
                MemcacheResponse::NoReply
            } else {
                MemcacheResponse::NotStored
            }
        }
    }

    fn delete(&mut self, key: &[u8], noreply: bool) -> MemcacheResponse {
        increment_counter!(&Stat::Delete);
        if self.data.delete(key) {
            increment_counter!(&Stat::DeleteDeleted);
            if noreply {
                MemcacheResponse::NoReply
            } else {
                MemcacheResponse::Deleted
            }
        } else {
            increment_counter!(&Stat::DeleteNotfound);
            if noreply {
                MemcacheResponse::NoReply
            } else {
                MemcacheResponse::NotFound
            }
        }
    }

    fn cas(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
        cas: u64,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Cas);
        let ttl = self.get_ttl(expiry);
        let value = value.unwrap_or(b"");
        match self.data.cas(
            key,
            value,
            Some(&flags.to_be_bytes()),
            CoarseDuration::from_secs(ttl),
            cas as u32,
        ) {
            Ok(_) => {
                increment_counter!(&Stat::CasStored);
                if noreply {
                    MemcacheResponse::NoReply
                } else {
                    MemcacheResponse::Stored
                }
            }
            Err(SegCacheError::NotFound) => {
                increment_counter!(&Stat::CasNotfound);
                if noreply {
                    MemcacheResponse::NoReply
                } else {
                    MemcacheResponse::NotFound
                }
            }
            Err(SegCacheError::Exists) => {
                increment_counter!(&Stat::CasExists);
                if noreply {
                    MemcacheResponse::NoReply
                } else {
                    MemcacheResponse::Exists
                }
            }
            Err(_) => {
                increment_counter!(&Stat::CasEx);
                if noreply {
                    MemcacheResponse::NoReply
                } else {
                    MemcacheResponse::NotStored
                }
            }
        }
    }
}

// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_time::{CoarseDuration, Duration};
use serde::{Deserialize, Serialize};

use std::time::SystemTime;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TimeType {
    Unix = 0,
    Delta = 1,
    Memcache = 2,
}

pub struct Expiry {
    expiry: u32,
    time_type: TimeType,
}

impl Expiry {
    pub fn new(expiry: u32, time_type: TimeType) -> Self {
        Self { expiry, time_type }
    }

    // TODO(bmartin): this conversion can be made more efficient
    pub fn as_secs(&self) -> u32 {
        match self.time_type {
            TimeType::Unix => {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                self.expiry.wrapping_sub(epoch)
            }
            TimeType::Delta => self.expiry,
            TimeType::Memcache => {
                if self.expiry < 60 * 60 * 24 * 30 {
                    self.expiry
                } else {
                    let epoch = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as u32;
                    self.expiry.wrapping_sub(epoch)
                }
            }
        }
    }

    pub fn from_memcache(expiry: u32) -> Expiry {
        Self {
            expiry,
            time_type: TimeType::Memcache,
        }
    }

    pub fn from_delta(expiry: u32) -> Expiry {
        Self {
            expiry,
            time_type: TimeType::Delta,
        }
    }

    pub fn from_unix(expiry: u32) -> Expiry {
        Self {
            expiry,
            time_type: TimeType::Unix,
        }
    }

    pub fn as_duration(&self) -> Duration {
        Duration::from_secs(self.as_secs().into())
    }

    pub fn as_coarse_duration(&self) -> CoarseDuration {
        CoarseDuration::from_secs(self.as_secs())
    }
}

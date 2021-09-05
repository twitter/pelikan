// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use rustcommon_fastmetrics::*;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter};

pub use macros::to_lowercase;
pub use rustcommon_metrics::{metric, Counter, Gauge};

pub type Metrics = rustcommon_fastmetrics::Metrics<Stat>;

#[doc(hidden)]
pub extern crate rustcommon_metrics;

#[macro_export]
macro_rules! pelikan_metrics {
    {$(
        $( #[ $attr:meta ] )*
        $vis:vis static $name:ident : $ty:ty ;
    )*} => {$(
        #[$crate::metric(
            name = $crate::to_lowercase!($name),
            crate = $crate::rustcommon_metrics
        )]
        $( #[ $attr ] )*
        $vis static $name : $ty = <$ty>::new();
    )*};
}

/// Creates a test that verifies that no two metrics have the same name.
#[macro_export]
macro_rules! test_no_duplicates {
    () => {
        #[cfg(test)]
        mod __metrics_tests {
            #[test]
            fn assert_no_duplicate_metric_names() {
                use $crate::rustcommon_metrics::*;
                use std::collections::HashSet;

                let mut seen = HashSet::new();
                for metric in metrics().static_metrics() {
                    let name = metric.name();
                    assert!(seen.insert(name), "found duplicate metric name '{}'", name);
                }
            }
        }
    };
}

// As a temporary work-around for requiring all metrics to have a common type,
// we combine server and storage metrics here.

/// Defines various statistics
#[derive(Copy, Clone, Debug, AsRefStr, EnumIter)]
#[strum(serialize_all = "snake_case")]
pub enum Stat {
    // server/twemcache-rs
    Add,
    AddNotstored,
    AddStored,
    AdminEventError,
    AdminEventLoop,
    AdminEventRead,
    AdminEventTotal,
    AdminEventWrite,
    AdminRequestParse,
    AdminRequestParseEx,
    AdminResponseCompose,
    AdminResponseComposeEx,
    Cas,
    CasNotfound,
    CasExists,
    CasEx,
    CasStored,
    Delete,
    DeleteDeleted,
    DeleteNotfound,
    Get,
    GetKey,
    GetKeyHit,
    GetKeyMiss,
    GetEx,
    Gets,
    GetsKey,
    GetsKeyHit,
    GetsKeyMiss,
    GetsEx,
    Set,
    SetNotstored,
    SetStored,
    SetEx,
    Pid,
    ProcessReq,
    ProcessEx,
    ProcessServerEx,
    Replace,
    ReplaceNotstored,
    ReplaceStored,
    RequestParse,
    RequestParseEx,
    ResponseCompose,
    ResponseComposeEx,
    RuUtime,
    RuStime,
    RuMaxrss,
    RuIxrss,
    RuIdrss,
    RuIsrss,
    RuMinflt,
    RuMajflt,
    RuNswap,
    RuInblock,
    RuOublock,
    RuMsgsnd,
    RuMsgrcv,
    RuNsignals,
    RuNvcsw,
    RuNivcsw,
    ServerEventError,
    ServerEventLoop,
    ServerEventRead,
    ServerEventTotal,
    ServerEventWrite,
    SessionRecv,
    SessionRecvByte,
    SessionRecvEx,
    SessionSend,
    SessionSendByte,
    SessionSendEx,
    StorageEventLoop,
    TcpAccept,
    TcpAcceptEx,
    TcpClose,
    TcpConnect,
    TcpConnectEx,
    TcpRecv,
    TcpRecvByte,
    TcpRecvEx,
    TcpReject,
    TcpRejectEx,
    TcpSend,
    TcpSendByte,
    TcpSendEx,
    TcpSendPartial,
    WorkerEventError,
    WorkerEventLoop,
    WorkerEventRead,
    WorkerEventTotal,
    WorkerEventWake,
    WorkerEventWrite,

    // storage/segcache
    ExpireTime,       // total time spent in expiration, nanoseconds
    HashLookup,       // total number of hash lookups
    HashInsert,       // total number of inserts
    HashInsertEx,     // caused by insert exceptions, typically ENOSPC
    HashRemove,       // number of hash removals
    HashTagCollision, // number of collisions
    HashArrayAlloc,
    ItemCurrent,
    ItemCurrentBytes,
    ItemAlloc,
    ItemAllocEx,
    ItemCompacted, // TODO(bmartin): better name?
    ItemDead,
    ItemDeadBytes,
    ItemDelete,
    ItemEvict,
    ItemExpire,
    ItemRelink,
    ItemReplace,
    SegmentRequest,
    SegmentRequestEx,
    SegmentReturn,
    SegmentEvict,
    SegmentEvictRetry,
    SegmentEvictEx,
    SegmentExpire,
    SegmentMerge,
    SegmentCurrent,
    SegmentFree,
}

#[allow(clippy::from_over_into)]
impl Into<usize> for Stat {
    fn into(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl Metric for Stat {
    fn source(&self) -> Source {
        match self {
            Stat::Pid
            | Stat::RuMaxrss
            | Stat::RuIxrss
            | Stat::RuIdrss
            | Stat::RuIsrss
            | Stat::SegmentCurrent
            | Stat::SegmentFree
            | Stat::ItemCurrent
            | Stat::ItemCurrentBytes
            | Stat::ItemDead
            | Stat::ItemDeadBytes => Source::Gauge,
            _ => Source::Counter,
        }
    }

    fn index(&self) -> usize {
        (*self).into()
    }
}

pub fn init() {
    let metrics: Vec<Stat> = Stat::iter().collect();
    MetricsBuilder::<Stat>::new()
        .metrics(&metrics)
        .build()
        .unwrap();

    set_gauge!(&Stat::Pid, std::process::id().into());
}
